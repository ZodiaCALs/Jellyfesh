//! The bridge between the core and GTK's main loop.
//!
//! * Events: the core delivers them on its own threads; one forwarding
//!   thread blocks on the core's channel and hands them to the main loop,
//!   which wakes only when an event arrives.
//! * Queries (`albums`, `tracks`, …) run on a worker thread, never on the main
//!   thread, so a long list or a busy library never blocks drawing. Results
//!   come back to the main loop as futures.
//! * Commands are fire-and-forget and go straight to the core.

use std::sync::Arc;
use std::thread::JoinHandle;

use crossbeam_channel::Sender;
use jf_core::{Command, CommandId, Config, Core, CoreError, Event};

type Job = Box<dyn FnOnce(&Core) + Send>;

pub struct Bridge {
    core: Arc<Core>,
    jobs: Option<Sender<Job>>,
    threads: Vec<JoinHandle<()>>,
}

impl std::fmt::Debug for Bridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bridge").finish_non_exhaustive()
    }
}

impl Bridge {
    /// Starts the core and the helper threads. Returns the bridge and the
    /// event stream for the main loop; the caller reads the snapshot after
    /// this, so no event between the two is lost.
    pub fn start(config: Config) -> Result<(Bridge, async_channel::Receiver<Event>), CoreError> {
        let core = Arc::new(Core::start(config)?);
        let rx = core.subscribe();
        let (tx, events) = async_channel::unbounded();
        let mut threads = Vec::new();
        threads.push(
            std::thread::Builder::new()
                .name("jf-gtk-events".into())
                .spawn(move || {
                    while let Ok(event) = rx.recv() {
                        if tx.send_blocking(event).is_err() {
                            break;
                        }
                    }
                })?,
        );
        let (jobs, job_rx) = crossbeam_channel::unbounded::<Job>();
        let worker_core = core.clone();
        threads.push(
            std::thread::Builder::new()
                .name("jf-gtk-query".into())
                .spawn(move || {
                    for job in job_rx {
                        job(&worker_core);
                    }
                })?,
        );
        Ok((
            Bridge {
                core,
                jobs: Some(jobs),
                threads,
            },
            events,
        ))
    }

    /// Sends a command; failures come back as `Event::CommandFailed`.
    pub fn send(&self, command: Command) -> CommandId {
        self.core.send(command)
    }

    /// The core, for cheap getters (`snapshot`) and helper threads.
    pub fn core(&self) -> &Arc<Core> {
        &self.core
    }

    /// Runs `job` on the query thread (dropped if the bridge is stopping).
    pub fn run(&self, job: impl FnOnce(&Core) + Send + 'static) {
        if let Some(jobs) = &self.jobs {
            let _ = jobs.send(Box::new(job));
        }
    }

    /// Stops the helper threads and the core, which saves the session.
    pub fn shutdown(mut self) {
        self.jobs = None;
        // The query thread ends once its channel closes; the event thread
        // ends when the core stops and drops its senders.
        let core = self.core;
        let mut threads = std::mem::take(&mut self.threads);
        if let Some(query) = threads.pop() {
            let _ = query.join();
        }
        match Arc::try_unwrap(core) {
            Ok(core) => core.shutdown(),
            Err(shared) => drop(shared), // the last holder's drop saves the session
        }
        for t in threads {
            let _ = t.join();
        }
    }
}
