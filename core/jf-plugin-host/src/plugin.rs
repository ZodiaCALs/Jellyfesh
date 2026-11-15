//! An installed plugin: its package, services, pending calls and live
//! instance.

use std::collections::VecDeque;
use std::sync::Arc;

use parking_lot::Mutex;
use wasmtime::Store;

use crate::bindings::Source;
use crate::bindings::remote::RemoteControl;
use crate::error::{CallError, HostError};
use crate::manifest::Manifest;
use crate::runtime::{Budget, Exports, HostState, Instance, Prepared, Protected, Runtime};
use crate::schema::SettingsSchema;
use crate::services::Services;
use crate::types::{Capability, SignatureStatus};

/// Work for a plugin, run on a pool worker with exclusive access to the
/// plugin's instance.
pub(crate) type Job = Box<dyn FnOnce(&mut JobContext<'_>) + Send>;

/// Pending calls and the live instance of one plugin.
#[derive(Default)]
pub(crate) struct Slot {
    pub queue: VecDeque<Job>,
    /// In the pool's ready queue or running on a worker.
    pub scheduled: bool,
    /// Uninstalled: accept no more work.
    pub closed: bool,
    /// Taken by the worker while a job runs.
    pub instance: Option<Instance>,
}

impl std::fmt::Debug for Slot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Slot")
            .field("queued", &self.queue.len())
            .field("scheduled", &self.scheduled)
            .field("closed", &self.closed)
            .field("live", &self.instance.is_some())
            .finish()
    }
}

/// An installed plugin.
pub(crate) struct Plugin {
    pub manifest: Manifest,
    pub schema: SettingsSchema,
    pub signature: SignatureStatus,
    pub icon: Option<Vec<u8>>,
    pub services: Arc<Services>,
    pub slot: Mutex<Slot>,
    wasm: Vec<u8>,
    prepared: Mutex<Option<Prepared>>,
}

impl std::fmt::Debug for Plugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Plugin")
            .field("id", &self.manifest.id)
            .field("version", &self.manifest.version.to_string())
            .field("slot", &*self.slot.lock())
            .field("compiled", &self.prepared.lock().is_some())
            .finish_non_exhaustive()
    }
}

impl Plugin {
    pub(crate) fn new(
        manifest: Manifest,
        schema: SettingsSchema,
        signature: SignatureStatus,
        icon: Option<Vec<u8>>,
        wasm: Vec<u8>,
        prepared: Option<Prepared>,
        services: Arc<Services>,
    ) -> Plugin {
        Plugin {
            manifest,
            schema,
            signature,
            icon,
            services,
            slot: Mutex::new(Slot::default()),
            wasm,
            prepared: Mutex::new(prepared),
        }
    }

    pub(crate) fn id(&self) -> &str {
        &self.manifest.id
    }

    /// The compiled, linked component (compiled on first use).
    fn prepared(&self, runtime: &Runtime) -> Result<Prepared, HostError> {
        let mut prepared = self.prepared.lock();
        if let Some(pre) = prepared.as_ref() {
            return Ok(pre.clone());
        }
        let started = std::time::Instant::now();
        let pre = runtime.prepare(
            &self.wasm,
            self.manifest.capabilities.contains(&Capability::Remote),
        )?;
        tracing::debug!(plugin = %self.id(), elapsed = ?started.elapsed(), "compiled plugin");
        *prepared = Some(pre.clone());
        Ok(pre)
    }

    /// Counts a limit violation; turns the plugin off after too many in a row.
    fn record_violation(&self, error: &CallError, runtime: &Runtime) {
        if !error.is_violation() {
            return;
        }
        let limit = runtime.limits.strikes;
        let disabled = self.services.update_state(|state| {
            state.strikes += 1;
            if state.strikes >= limit && state.enabled {
                state.enabled = false;
                state.disabled_reason = Some(format!(
                    "Turned off after breaking its limits {limit} times in a row (last: {error})"
                ));
                true
            } else {
                false
            }
        });
        if disabled {
            tracing::warn!(plugin = %self.id(), %error, "plugin turned off after repeated violations");
        } else {
            tracing::info!(plugin = %self.id(), %error, "plugin call stopped; instance restarted");
        }
    }
}

/// What a job sees while it runs.
pub(crate) struct JobContext<'a> {
    pub runtime: &'a Runtime,
    pub plugin: &'a Arc<Plugin>,
    pub instance: &'a mut Option<Instance>,
}

impl JobContext<'_> {
    /// Calls into the plugin under `budget`, instantiating it first if
    /// needed. A trap or violation drops the instance (the next call starts
    /// a fresh one) and counts as a strike.
    pub(crate) fn call<R>(
        &mut self,
        budget: Budget,
        protected: Protected,
        f: impl FnOnce(&Source, &mut Store<HostState>) -> wasmtime::Result<R>,
    ) -> Result<R, CallError> {
        self.run(budget, protected, |exports, store| f(&exports.source, store))
    }

    /// Calls the plugin's `remote` export like [`JobContext::call`]. Plugins
    /// whose manifest does not declare `remote` have none.
    pub(crate) fn call_remote<R>(
        &mut self,
        budget: Budget,
        f: impl FnOnce(&RemoteControl, &mut Store<HostState>) -> wasmtime::Result<R>,
    ) -> Result<R, CallError> {
        self.ensure_instance()?;
        if !self.instance.as_ref().is_some_and(Instance::has_remote) {
            return Err(CallError::NotDeclared("remote".into()));
        }
        self.run(budget, Protected::default(), |exports, store| {
            f(exports.remote.as_ref().expect("checked above"), store)
        })
    }

    fn ensure_instance(&mut self) -> Result<(), CallError> {
        if self.instance.is_some() {
            return Ok(());
        }
        let pre = self
            .plugin
            .prepared(self.runtime)
            .map_err(|e| CallError::Load(e.to_string()))?;
        match self.runtime.instantiate(&pre, &self.plugin.services) {
            Ok(instance) => {
                *self.instance = Some(instance);
                Ok(())
            }
            Err(error) => {
                self.plugin.record_violation(&error, self.runtime);
                Err(error)
            }
        }
    }

    fn run<R>(
        &mut self,
        budget: Budget,
        protected: Protected,
        f: impl FnOnce(&Exports, &mut Store<HostState>) -> wasmtime::Result<R>,
    ) -> Result<R, CallError> {
        self.ensure_instance()?;
        let instance = self.instance.as_mut().expect("instantiated above");
        let result = instance.call(budget, protected, f);
        match &result {
            Ok(_) => self.plugin.services.reset_strikes(),
            Err(error) => {
                *self.instance = None;
                self.plugin.record_violation(error, self.runtime);
            }
        }
        result
    }

    /// Drops the live instance so the next call starts fresh (after a
    /// password exchange, so the password does not linger in its memory).
    pub(crate) fn restart(&mut self) {
        *self.instance = None;
    }

    /// Whether an instance is live.
    pub(crate) fn is_live(&self) -> bool {
        self.instance.is_some()
    }
}
