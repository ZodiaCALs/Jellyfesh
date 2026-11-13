//! Unified search: the library at once, then every source in parallel, each
//! with its own deadline, merged as the answers come in.
//!
//! One thread (`jf-search`) runs one search at a time. The actor sends the
//! library's answer with the job (it has already published it); the thread
//! waits [`SETTLE`] for a newer search (typing), asks every source, and
//! publishes each source's section when it answers, fails or runs out of
//! time. Every answer is merged with the ones before it
//! ([`convert::sections`]): a source's section holds only what the sources
//! before it do not have, and an earlier section whose rows changed (a
//! library track that this source also has gains it in `also_in`) is
//! published again just before. A newer search ends the current one: its
//! late answers are dropped, and [`Ctx::emit_search`] drops anything older
//! that is still on its way.

use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, RecvTimeoutError, Select, Sender};
use jf_plugin_host::{ItemPage, PageRequest, Reply, SearchFilters};

use super::Ctx;
use super::convert::{self, Answer};
use crate::types::{CommandId, SearchUpdate, SourceItem};

/// How long a search waits for a newer one before asking the sources.
const SETTLE: Duration = Duration::from_millis(100);
/// Results asked of each source.
const PER_SOURCE: u32 = 50;

/// One search for the sources.
#[derive(Debug)]
pub(crate) struct SearchJob {
    pub request: CommandId,
    pub query: String,
    /// The library's answer when the search covers the library too.
    pub library: Option<Answer>,
    /// The plugins to ask, in order.
    pub plugins: Vec<String>,
}

pub(crate) fn spawn(ctx: Arc<Ctx>) -> std::io::Result<Sender<SearchJob>> {
    let (tx, rx) = crossbeam_channel::unbounded::<SearchJob>();
    std::thread::Builder::new()
        .name("jf-search".into())
        .spawn(move || run(&ctx, &rx))?;
    Ok(tx)
}

fn run(ctx: &Ctx, jobs: &Receiver<SearchJob>) {
    let mut next = None;
    loop {
        let mut job = match next.take() {
            Some(job) => job,
            None => match jobs.recv() {
                Ok(job) => job,
                Err(_) => return,
            },
        };
        // Typing sends a search per key: only the last one goes out.
        loop {
            match jobs.recv_timeout(SETTLE) {
                Ok(newer) => job = newer,
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
        next = search(ctx, &job, jobs);
    }
}

/// Runs one search; returns a newer job if one arrived meanwhile.
fn search(ctx: &Ctx, job: &SearchJob, jobs: &Receiver<SearchJob>) -> Option<SearchJob> {
    let mut answers: Vec<Answer> = job.library.iter().cloned().collect();
    // The section last published for each answer (the actor published the
    // library's).
    let mut sent: Vec<Vec<SourceItem>> = answers.iter().map(|a| a.rows.clone()).collect();
    let mut pending: Vec<(String, Reply<ItemPage>)> = job
        .plugins
        .iter()
        .map(|plugin| {
            let reply = ctx.host.search(
                plugin,
                &job.query,
                SearchFilters { kinds: Vec::new() },
                PageRequest {
                    cursor: None,
                    limit: PER_SOURCE,
                },
            );
            (plugin.clone(), reply)
        })
        .collect();
    let deadline = Instant::now() + ctx.search_timeout;
    while !pending.is_empty() {
        let left = deadline.saturating_duration_since(Instant::now());
        let (index, result) = {
            let mut select = Select::new();
            select.recv(jobs);
            for (_, reply) in &pending {
                select.recv(reply);
            }
            match select.select_timeout(left) {
                Err(_) => (None, None),
                Ok(operation) if operation.index() == 0 => {
                    // A newer search (or none: the core is shutting down).
                    return operation.recv(jobs).ok();
                }
                Ok(operation) => {
                    let index = operation.index() - 1;
                    (Some(index), Some(operation.recv(&pending[index].1)))
                }
            }
        };
        let (Some(index), Some(result)) = (index, result) else {
            // Out of time: the sources that have not answered are reported
            // late, the last of them with `done`.
            let late: Vec<String> = pending.drain(..).map(|(plugin, _)| plugin).collect();
            for (n, plugin) in late.iter().enumerate() {
                let error = format!("{} did not answer in time", ctx.name(plugin));
                ctx.emit_search(update(job, plugin, Vec::new(), Some(error), n + 1 == late.len()));
            }
            return None;
        };
        let (plugin, _) = pending.remove(index);
        let done = pending.is_empty();
        match result {
            Ok(Ok(page)) => {
                ctx.remember(&plugin, &page.items);
                answers.push(Answer::plugin(&plugin, page.items));
                let mut sections = convert::sections(&answers);
                let own = sections.pop().unwrap_or_default();
                for (answer, (section, last)) in answers.iter().zip(sections.into_iter().zip(sent.iter_mut()))
                {
                    if section != *last {
                        ctx.emit_search(update(job, &answer.source, section.clone(), None, false));
                        *last = section;
                    }
                }
                sent.push(own.clone());
                ctx.emit_search(update(job, &plugin, own, None, done));
            }
            Ok(Err(error)) => {
                ctx.note(&error);
                ctx.emit_search(update(job, &plugin, Vec::new(), Some(error.to_string()), done));
            }
            Err(_) => {
                let error = "the plugin host has shut down".to_owned();
                ctx.emit_search(update(job, &plugin, Vec::new(), Some(error), done));
            }
        }
    }
    None
}

fn update(
    job: &SearchJob,
    source: &str,
    items: Vec<SourceItem>,
    error: Option<String>,
    done: bool,
) -> SearchUpdate {
    SearchUpdate {
        request: job.request,
        query: job.query.clone(),
        source: source.to_owned(),
        items,
        error,
        done,
    }
}
