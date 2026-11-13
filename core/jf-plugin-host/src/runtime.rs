//! The wasmtime side: engine configuration, the per-store state that host
//! imports run against, sandbox limits, and instance lifecycles.
//!
//! * One [`Engine`] per host: Cranelift, component model, epoch interruption;
//!   no WASI, no async, no fuel. Components are compiled when a plugin is
//!   installed and again, lazily, on its first call after the app starts:
//!   `Component::deserialize` is `unsafe` (it trusts the artifact), and the
//!   workspace denies `unsafe_code`, so v0.0.3 does not cache compiled code
//!   on disk (ADR-0003 step 3 waits for that).
//! * One long-lived instance per plugin, created on first use from a
//!   pre-linked [`SourcePre`]. A trap or limit violation drops it; the next
//!   call instantiates a fresh one ("restarted lazily").
//! * CPU: an epoch ticker advances the engine every [`EPOCH_TICK`] while guest
//!   code runs (it parks otherwise, so an idle host wakes nobody); the store's
//!   deadline callback runs on every tick while guest code executes and
//!   compares the call's guest time (wall time minus time spent in host
//!   imports) with the budget. The same callback enforces the wall-clock
//!   deadline; host imports bound their own waits by it.
//! * Memory: `StoreLimits` (64 MiB, one table of at most 10 000 elements)
//!   wrapped so a refused growth traps with a recognisable error.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use wasmtime::component::{Component, HasSelf, Linker};
use wasmtime::{Config, Engine, ResourceLimiter, Store, StoreLimits, StoreLimitsBuilder, UpdateDeadline};
use zeroize::Zeroizing;

use crate::bindings::{Source, SourcePre};
use crate::error::{CallError, HostError};
use crate::http::NetBudget;
use crate::services::Services;

/// How often the epoch advances.
pub const EPOCH_TICK: Duration = Duration::from_millis(5);

/// Sandbox limits (ADR-0004). Tests shrink some of them to stay fast.
#[derive(Debug, Clone)]
pub struct Limits {
    /// CPU per call.
    pub cpu: Duration,
    /// CPU for `auth.begin` (and for instantiation).
    pub cpu_long: Duration,
    /// Wall-clock time per call.
    pub wall: Duration,
    /// Wall-clock time per login step.
    pub wall_login: Duration,
    /// Linear memory per instance, in bytes.
    pub memory: usize,
    /// Elements per table.
    pub table_elements: usize,
    /// Consecutive violations that turn a plugin off.
    pub strikes: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            cpu: Duration::from_millis(200),
            cpu_long: Duration::from_secs(2),
            wall: Duration::from_secs(10),
            wall_login: Duration::from_secs(30),
            memory: 64 * 1024 * 1024,
            table_elements: 10_000,
            strikes: 3,
        }
    }
}

/// CPU and wall-clock budget of one call.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Budget {
    pub cpu: Duration,
    pub wall: Duration,
}

impl Budget {
    pub(crate) fn call(limits: &Limits) -> Budget {
        Budget {
            cpu: limits.cpu,
            wall: limits.wall,
        }
    }
    pub(crate) fn start(limits: &Limits) -> Budget {
        Budget {
            cpu: limits.cpu_long,
            wall: limits.wall,
        }
    }
    pub(crate) fn auth_begin(limits: &Limits) -> Budget {
        Budget {
            cpu: limits.cpu_long,
            wall: limits.wall_login,
        }
    }
    pub(crate) fn auth_step(limits: &Limits) -> Budget {
        Budget {
            cpu: limits.cpu,
            wall: limits.wall_login,
        }
    }
}

/// A limit the guest ran into; carried by the trap that stops it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum Violation {
    #[error("the CPU budget is used up")]
    Cpu,
    #[error("the wall-clock deadline has passed")]
    Wall,
    #[error("the memory limit is reached")]
    Memory,
}

/// Values a login submit protects from storage and logs.
#[derive(Debug, Default)]
pub(crate) struct Protected {
    /// Secret field values as typed (wiped when dropped).
    pub values: Vec<Zeroizing<String>>,
    /// Also refuse them in `kv.set-secret` (password-exchange).
    pub keystore: bool,
}

/// Bookkeeping for the call in progress.
#[derive(Debug)]
pub(crate) struct CallState {
    started: Instant,
    budget: Budget,
    /// Time spent inside host imports during this call.
    pub host_time: Duration,
    pub net: NetBudget,
    pub protected: Protected,
    /// The last `panic: …` line the plugin logged.
    pub panic_message: Option<String>,
    pub violation: Option<Violation>,
}

impl CallState {
    fn new(budget: Budget) -> CallState {
        let started = Instant::now();
        CallState {
            started,
            budget,
            host_time: Duration::ZERO,
            net: NetBudget {
                deadline: started + budget.wall,
                received: 0,
            },
            protected: Protected::default(),
            panic_message: None,
            violation: None,
        }
    }

    fn check(&self, now: Instant) -> Option<Violation> {
        let elapsed = now.saturating_duration_since(self.started);
        if elapsed > self.budget.wall {
            Some(Violation::Wall)
        } else if elapsed.saturating_sub(self.host_time) > self.budget.cpu {
            Some(Violation::Cpu)
        } else {
            None
        }
    }
}

/// `StoreLimits` that remember why they refused.
#[derive(Debug)]
pub(crate) struct Limiter {
    inner: StoreLimits,
}

impl ResourceLimiter for Limiter {
    fn memory_growing(
        &mut self,
        current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        if self.inner.memory_growing(current, desired, maximum)? {
            Ok(true)
        } else {
            Err(wasmtime::Error::new(Violation::Memory))
        }
    }

    fn table_growing(
        &mut self,
        current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        if self.inner.table_growing(current, desired, maximum)? {
            Ok(true)
        } else {
            Err(wasmtime::Error::new(Violation::Memory))
        }
    }

    fn instances(&self) -> usize {
        self.inner.instances()
    }

    fn tables(&self) -> usize {
        self.inner.tables()
    }

    fn memories(&self) -> usize {
        self.inner.memories()
    }
}

/// The data of a plugin's store: what host imports run against.
#[derive(Debug)]
pub(crate) struct HostState {
    pub services: Arc<Services>,
    pub limiter: Limiter,
    pub call: CallState,
    pub clock_base: Instant,
}

impl HostState {
    /// Runs `f` as host time: it does not count against the CPU budget.
    pub(crate) fn host<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let started = Instant::now();
        let result = f(self);
        self.call.host_time += started.elapsed();
        result
    }
}

/// Engine-wide pieces shared by every plugin. The workers hold it, so the
/// epoch keeps ticking (and a running guest stays interruptible) until the
/// last worker has finished, even after the host itself is dropped.
pub(crate) struct Runtime {
    pub engine: Engine,
    pub linker: Linker<HostState>,
    pub limits: Limits,
    pub clock_base: Instant,
    ticker: EpochTicker,
}

impl std::fmt::Debug for Runtime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Runtime")
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

impl Runtime {
    pub(crate) fn new(limits: Limits) -> Result<Runtime, HostError> {
        let mut config = Config::new();
        config.epoch_interruption(true);
        config.wasm_component_model(true);
        config.cranelift_opt_level(wasmtime::OptLevel::Speed);
        let engine = Engine::new(&config).map_err(|e| HostError::Other(format!("wasmtime: {e}")))?;
        let mut linker = Linker::<HostState>::new(&engine);
        Source::add_to_linker::<HostState, HasSelf<HostState>>(&mut linker, |state| state)
            .map_err(|e| HostError::Other(format!("linking host imports: {e}")))?;
        let ticker = EpochTicker::start(engine.clone())?;
        Ok(Runtime {
            engine,
            linker,
            limits,
            clock_base: Instant::now(),
            ticker,
        })
    }

    /// Compiles a component and links it against the host's imports. Fails
    /// for invalid components and for components that import anything the
    /// host does not provide (WASI, another WIT version) or lack an export.
    pub(crate) fn prepare(&self, wasm: &[u8]) -> Result<SourcePre<HostState>, HostError> {
        let component =
            Component::new(&self.engine, wasm).map_err(|e| HostError::InvalidComponent(format!("{e:#}")))?;
        let pre = self
            .linker
            .instantiate_pre(&component)
            .map_err(|e| HostError::Incompatible(format!("{e:#}")))?;
        SourcePre::new(pre).map_err(|e| HostError::Incompatible(format!("{e:#}")))
    }

    /// Instantiates a plugin and delivers its settings.
    pub(crate) fn instantiate(
        &self,
        pre: &SourcePre<HostState>,
        services: &Arc<Services>,
    ) -> Result<Instance, CallError> {
        let mut store = Store::new(
            &self.engine,
            HostState {
                services: services.clone(),
                limiter: Limiter {
                    inner: StoreLimitsBuilder::new()
                        .memory_size(self.limits.memory)
                        .table_elements(self.limits.table_elements)
                        .instances(32)
                        .tables(16)
                        .memories(4)
                        .build(),
                },
                call: CallState::new(Budget::start(&self.limits)),
                clock_base: self.clock_base,
            },
        );
        store.limiter(|state| &mut state.limiter);
        store.epoch_deadline_callback(|mut ctx| {
            let state = ctx.data_mut();
            match state.call.check(Instant::now()) {
                None => Ok(UpdateDeadline::Continue(1)),
                Some(violation) => {
                    state.call.violation = Some(violation);
                    Err(wasmtime::Error::new(violation))
                }
            }
        });
        let mut instance = Instance {
            store,
            source: None,
            ticker: self.ticker.shared.clone(),
        };
        // Start functions and `settings-changed` are guest code too.
        let _ticking = instance.ticker.ticking();
        instance.begin(Budget::start(&self.limits), Protected::default());
        let source = pre
            .instantiate(&mut instance.store)
            .map_err(|e| instance.classify(e))?;
        let settings = services.settings_json();
        match source
            .jellyfesh_plugin_meta()
            .call_settings_changed(&mut instance.store, &settings)
        {
            Ok(Ok(())) => {}
            Ok(Err(error)) => tracing::warn!(
                plugin = %services.id,
                error = %crate::error::plugin_error_text(&error.into()),
                "the plugin did not accept its settings"
            ),
            Err(trap) => return Err(instance.classify(trap)),
        }
        instance.source = Some(source);
        Ok(instance)
    }
}

/// A live plugin instance.
pub(crate) struct Instance {
    pub store: Store<HostState>,
    source: Option<Source>,
    ticker: Arc<TickerShared>,
}

impl std::fmt::Debug for Instance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Instance")
            .field("plugin", &self.store.data().services.id)
            .finish_non_exhaustive()
    }
}

impl Instance {
    fn begin(&mut self, budget: Budget, protected: Protected) {
        self.store.data_mut().call = CallState::new(budget);
        self.store.data_mut().call.protected = protected;
        self.store.set_epoch_deadline(1);
    }

    /// Runs one export under `budget`. On a trap the caller must drop the
    /// instance.
    pub(crate) fn call<R>(
        &mut self,
        budget: Budget,
        protected: Protected,
        f: impl FnOnce(&Source, &mut Store<HostState>) -> wasmtime::Result<R>,
    ) -> Result<R, CallError> {
        self.begin(budget, protected);
        let _ticking = self.ticker.ticking();
        let Instance { store, source, .. } = self;
        let result = f(source.as_ref().expect("instantiated"), store);
        // Wipe protected values now, not when the next call begins.
        self.store.data_mut().call.protected = Protected::default();
        result.map_err(|e| self.classify(e))
    }

    /// Turns a trap into the error the caller sees.
    fn classify(&self, error: wasmtime::Error) -> CallError {
        let call = &self.store.data().call;
        let violation = error.downcast_ref::<Violation>().copied().or(call.violation);
        match violation {
            Some(Violation::Cpu) => CallError::CpuLimit,
            Some(Violation::Wall) => CallError::Timeout,
            Some(Violation::Memory) => CallError::MemoryLimit,
            None => {
                let what = match error.downcast_ref::<wasmtime::Trap>() {
                    Some(trap) => trap.to_string(),
                    None => format!("{error:#}"),
                };
                match &call.panic_message {
                    Some(panic) => CallError::Crashed(format!("{panic} ({what})")),
                    None => CallError::Crashed(what),
                }
            }
        }
    }
}

/// Advances the engine's epoch while guest code may run, until dropped.
///
/// The thread parks while no call is in progress ([`TickerShared::ticking`]
/// counts them), so a host with nothing to do wakes nobody: the app's idle
/// budget allows no timers.
#[derive(Debug)]
pub(crate) struct EpochTicker {
    shared: Arc<TickerShared>,
    thread: Option<JoinHandle<()>>,
}

/// What the ticker thread and the calls share.
#[derive(Debug, Default)]
pub(crate) struct TickerShared {
    state: parking_lot::Mutex<TickerState>,
    wake: parking_lot::Condvar,
    /// Epochs advanced so far.
    ticks: AtomicU64,
}

#[derive(Debug, Default)]
struct TickerState {
    /// Stretches of guest code in progress.
    active: usize,
    stop: bool,
}

impl TickerShared {
    /// Keeps the epoch advancing until the returned guard is dropped.
    pub(crate) fn ticking(self: &Arc<Self>) -> Ticking {
        self.state.lock().active += 1;
        self.wake.notify_one();
        Ticking(self.clone())
    }
}

/// Held around guest code: the epoch advances while any guard lives.
#[derive(Debug)]
pub(crate) struct Ticking(Arc<TickerShared>);

impl Drop for Ticking {
    fn drop(&mut self) {
        let mut state = self.0.state.lock();
        state.active = state.active.saturating_sub(1);
    }
}

impl EpochTicker {
    pub(crate) fn start(engine: Engine) -> Result<EpochTicker, HostError> {
        let shared = Arc::new(TickerShared::default());
        let thread = {
            let shared = shared.clone();
            std::thread::Builder::new()
                .name("jf-plugin-epoch".into())
                .spawn(move || {
                    loop {
                        {
                            let mut state = shared.state.lock();
                            while state.active == 0 && !state.stop {
                                shared.wake.wait(&mut state);
                            }
                            if state.stop {
                                return;
                            }
                        }
                        std::thread::sleep(EPOCH_TICK);
                        engine.increment_epoch();
                        shared.ticks.fetch_add(1, Ordering::Relaxed);
                    }
                })?
        };
        Ok(EpochTicker {
            shared,
            thread: Some(thread),
        })
    }

    /// Epochs advanced so far.
    #[cfg(test)]
    fn ticks(&self) -> u64 {
        self.shared.ticks.load(Ordering::Relaxed)
    }
}

impl Drop for EpochTicker {
    fn drop(&mut self) {
        self.shared.state.lock().stop = true;
        self.shared.wake.notify_all();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_epoch_ticker_parks_while_no_guest_code_runs() {
        let engine = Engine::new(Config::new().epoch_interruption(true)).unwrap();
        let ticker = EpochTicker::start(engine).unwrap();
        std::thread::sleep(EPOCH_TICK * 8);
        assert_eq!(ticker.ticks(), 0, "an idle host wakes nobody");

        let guard = ticker.shared.ticking();
        let started = Instant::now();
        while ticker.ticks() < 3 {
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "the epoch must advance while guest code runs"
            );
            std::thread::sleep(EPOCH_TICK);
        }
        drop(guard);
        // The tick in flight when the guard went may still land; then it parks again.
        std::thread::sleep(EPOCH_TICK * 3);
        let parked = ticker.ticks();
        std::thread::sleep(EPOCH_TICK * 8);
        assert_eq!(ticker.ticks(), parked);
    }
}
