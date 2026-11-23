//! Search by source (ADR-0024 §4): the library at once, then every source
//! in parallel, each with its own deadline. The sources whose terms allow it
//! are merged with the library as the answers come in; the others (Spotify,
//! QQ Music) answer in sections of their own (`merged` false), never
//! de-duplicated against the rest. Every section also carries the source's
//! own results kind by kind (`groups`), for its tab and Show More.
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
use crate::types::{CommandId, ItemKind, SearchGroup, SearchUpdate, SourceItem};

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
    /// The plugins to ask, in order, each with whether its terms let its
    /// results merge with others' (`content_policy.unified_search`).
    pub plugins: Vec<(String, bool)>,
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
    // The section last published for each merged answer (the actor published
    // the library's), and its groups.
    let mut sent: Vec<Vec<SourceItem>> = answers.iter().map(|a| a.rows.clone()).collect();
    let mut sent_groups: Vec<Vec<SearchGroup>> = answers.iter().map(|a| groups(&a.rows)).collect();
    let mut pending: Vec<(String, bool, Reply<ItemPage>)> = job
        .plugins
        .iter()
        .map(|(plugin, merged)| {
            let reply = ctx.host.search(
                plugin,
                &job.query,
                SearchFilters { kinds: Vec::new() },
                PageRequest {
                    cursor: None,
                    limit: PER_SOURCE,
                },
            );
            (plugin.clone(), *merged, reply)
        })
        .collect();
    let deadline = Instant::now() + ctx.search_timeout;
    while !pending.is_empty() {
        let left = deadline.saturating_duration_since(Instant::now());
        let (index, result) = {
            let mut select = Select::new();
            select.recv(jobs);
            for (_, _, reply) in &pending {
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
                    (Some(index), Some(operation.recv(&pending[index].2)))
                }
            }
        };
        let (Some(index), Some(result)) = (index, result) else {
            // Out of time: the sources that have not answered are reported
            // late, the last of them with `done`.
            let late: Vec<(String, bool)> = pending
                .drain(..)
                .map(|(plugin, merged, _)| (plugin, merged))
                .collect();
            for (n, (plugin, merged)) in late.iter().enumerate() {
                let error = format!("{} did not answer in time", ctx.name(plugin));
                ctx.emit_search(failed(job, plugin, *merged, error, n + 1 == late.len()));
            }
            return None;
        };
        let (plugin, merged, _) = pending.remove(index);
        let done = pending.is_empty();
        match result {
            Ok(Ok(page)) => {
                ctx.remember(&plugin, &page.items);
                let answer = Answer::plugin(&plugin, page.items, &ctx.links(&plugin));
                let groups = groups(&answer.rows);
                if !merged {
                    // A section of its own: everything it found, nothing merged.
                    let rows = answer.rows;
                    ctx.emit_search(update(job, &plugin, rows, false, groups, done));
                    continue;
                }
                answers.push(answer);
                let mut sections = convert::sections(&answers);
                let own = sections.pop().unwrap_or_default();
                for ((answer, (section, last)), groups) in answers
                    .iter()
                    .zip(sections.into_iter().zip(sent.iter_mut()))
                    .zip(sent_groups.iter())
                {
                    if section != *last {
                        let groups = groups.clone();
                        ctx.emit_search(update(job, &answer.source, section.clone(), true, groups, false));
                        *last = section;
                    }
                }
                sent.push(own.clone());
                sent_groups.push(groups.clone());
                ctx.emit_search(update(job, &plugin, own, true, groups, done));
            }
            Ok(Err(error)) => {
                ctx.note(&error);
                ctx.emit_search(failed(job, &plugin, merged, error.to_string(), done));
            }
            Err(_) => {
                let error = "the plugin host has shut down".to_owned();
                ctx.emit_search(failed(job, &plugin, merged, error, done));
            }
        }
    }
    None
}

/// A source's rows kind by kind, in the order Songs, Albums, Artists,
/// Playlists, Folders (kinds it did not find are left out).
///
/// Every kind comes from one query for now, so no group has a cursor of its
/// own yet (M2 asks each kind separately, with Show More).
pub(crate) fn groups(rows: &[SourceItem]) -> Vec<SearchGroup> {
    [
        ItemKind::Track,
        ItemKind::Album,
        ItemKind::Artist,
        ItemKind::Playlist,
        ItemKind::Folder,
    ]
    .into_iter()
    .filter_map(|kind| {
        let items: Vec<SourceItem> = rows.iter().filter(|r| r.kind == kind).cloned().collect();
        (!items.is_empty()).then_some(SearchGroup {
            kind,
            items,
            next: None,
            total: None,
        })
    })
    .collect()
}

fn update(
    job: &SearchJob,
    source: &str,
    items: Vec<SourceItem>,
    merged: bool,
    groups: Vec<SearchGroup>,
    done: bool,
) -> SearchUpdate {
    SearchUpdate {
        request: job.request,
        query: job.query.clone(),
        source: source.to_owned(),
        items,
        merged,
        groups,
        error: None,
        done,
    }
}

fn failed(job: &SearchJob, source: &str, merged: bool, error: String, done: bool) -> SearchUpdate {
    SearchUpdate {
        error: Some(error),
        ..update(job, source, Vec::new(), merged, Vec::new(), done)
    }
}
