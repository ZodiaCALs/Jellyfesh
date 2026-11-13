//! The worker pool that runs plugin calls.
//!
//! A plugin's calls run one at a time, in order, on whichever worker picks the
//! plugin up, and a plugin occupies at most one worker at a time: its
//! pending calls wait in its own queue, not in the pool's. So a plugin that
//! spins or waits on the network holds one worker at most, until its CPU
//! budget or deadline stops it, and every other plugin keeps the remaining
//! workers. After each call the plugin goes to the back of the ready queue,
//! so a busy plugin cannot starve the others. Callers never wait on the pool:
//! every call returns a channel.

use std::collections::VecDeque;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::thread::JoinHandle;

use parking_lot::{Condvar, Mutex};

use crate::plugin::{Job, JobContext, Plugin};
use crate::runtime::Runtime;

/// Workers in the pool (desktop and mobile alike for now).
pub const WORKERS: usize = 2;

#[derive(Debug, Default)]
struct Ready {
    plugins: VecDeque<Arc<Plugin>>,
    shutdown: bool,
}

#[derive(Debug)]
struct Shared {
    ready: Mutex<Ready>,
    wake: Condvar,
    runtime: Arc<Runtime>,
}

/// Hands jobs to the workers.
#[derive(Debug, Clone)]
pub(crate) struct Scheduler {
    shared: Arc<Shared>,
}

impl Scheduler {
    /// Starts `workers` threads.
    pub(crate) fn start(
        runtime: Arc<Runtime>,
        workers: usize,
    ) -> std::io::Result<(Scheduler, Vec<JoinHandle<()>>)> {
        let shared = Arc::new(Shared {
            ready: Mutex::new(Ready::default()),
            wake: Condvar::new(),
            runtime,
        });
        let mut threads = Vec::new();
        for n in 0..workers.max(1) {
            let shared = shared.clone();
            threads.push(
                std::thread::Builder::new()
                    .name(format!("jf-plugin-{n}"))
                    .spawn(move || work(&shared))?,
            );
        }
        Ok((Scheduler { shared }, threads))
    }

    /// Queues a job for `plugin`. Returns false (dropping the job, which
    /// disconnects its reply channel) if the plugin is uninstalled or the
    /// pool is shutting down.
    pub(crate) fn submit(&self, plugin: &Arc<Plugin>, job: Job) -> bool {
        let mut slot = plugin.slot.lock();
        if slot.closed || self.shared.ready.lock().shutdown {
            return false;
        }
        slot.queue.push_back(job);
        if !slot.scheduled {
            slot.scheduled = true;
            drop(slot);
            self.enqueue(plugin.clone());
        }
        true
    }

    fn enqueue(&self, plugin: Arc<Plugin>) {
        self.shared.ready.lock().plugins.push_back(plugin);
        self.shared.wake.notify_one();
    }

    /// Stops the workers once their current jobs end; queued jobs are dropped.
    pub(crate) fn shutdown(&self) {
        let mut ready = self.shared.ready.lock();
        ready.shutdown = true;
        let plugins: Vec<_> = ready.plugins.drain(..).collect();
        drop(ready);
        for plugin in plugins {
            let mut slot = plugin.slot.lock();
            slot.queue.clear();
            slot.scheduled = false;
        }
        self.shared.wake.notify_all();
    }
}

fn work(shared: &Arc<Shared>) {
    let scheduler = Scheduler {
        shared: shared.clone(),
    };
    loop {
        let plugin = {
            let mut ready = shared.ready.lock();
            loop {
                if ready.shutdown {
                    return;
                }
                if let Some(plugin) = ready.plugins.pop_front() {
                    break plugin;
                }
                shared.wake.wait(&mut ready);
            }
        };
        let (job, mut instance) = {
            let mut slot = plugin.slot.lock();
            (slot.queue.pop_front(), slot.instance.take())
        };
        if let Some(job) = job {
            let mut context = JobContext {
                runtime: &shared.runtime,
                plugin: &plugin,
                instance: &mut instance,
            };
            if catch_unwind(AssertUnwindSafe(|| job(&mut context))).is_err() {
                // A bug in the host, not the plugin: start the plugin afresh.
                tracing::error!(plugin = %plugin.id(), "a plugin job panicked in the host");
                instance = None;
            }
        }
        let mut slot = plugin.slot.lock();
        if slot.closed {
            slot.queue.clear();
            slot.scheduled = false;
            drop(instance);
            continue;
        }
        slot.instance = instance;
        if slot.queue.is_empty() {
            slot.scheduled = false;
        } else {
            drop(slot);
            scheduler.enqueue(plugin);
        }
    }
}
