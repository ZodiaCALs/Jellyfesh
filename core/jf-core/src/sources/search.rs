//! Search by source (ADR-0024 §4): the library at once, then every source
//! in parallel, each with its own deadline. Each source is asked for its
//! songs, albums, artists and playlists separately (`search-filters.kinds`,
//! 20 of each), so every kind has its own cursor for Show More
//! (`Command::SearchMore`). The sources whose terms allow it are merged with
//! the library as the answers come in, source by source; the others
//! (Spotify, QQ Music) answer in sections of their own (`merged` false),
//! never de-duplicated against the rest. Every section also carries the
//! source's own results kind by kind (`groups`), for its tab.
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
/// The kinds each source is asked for, each on its own, in its tab's order.
const KINDS: [ItemKind; 4] = [
    ItemKind::Track,
    ItemKind::Album,
    ItemKind::Artist,
    ItemKind::Playlist,
];
/// Results asked of each source per kind on the first page.
const PER_KIND: u32 = 20;

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

/// What a source answered so far, kind by kind.
struct Asked {
    plugin: String,
    /// Its terms let its results merge with others'.
    merged: bool,
    /// Kinds that have not answered yet.
    left: usize,
    /// The kinds that answered, with their pages.
    pages: Vec<(ItemKind, ItemPage)>,
    /// The first failure among its kinds.
    error: Option<String>,
    /// Its section went out.
    finished: bool,
}

/// The sections published so far for the merged answers (the library's
/// first, which the actor published), with their groups.
struct Merged {
    answers: Vec<Answer>,
    sent: Vec<Vec<SourceItem>>,
    sent_groups: Vec<Vec<SearchGroup>>,
}

/// Runs one search; returns a newer job if one arrived meanwhile.
fn search(ctx: &Ctx, job: &SearchJob, jobs: &Receiver<SearchJob>) -> Option<SearchJob> {
    let answers: Vec<Answer> = job.library.iter().cloned().collect();
    let mut merged = Merged {
        sent: answers.iter().map(|a| a.rows.clone()).collect(),
        sent_groups: answers.iter().map(|a| groups(&a.rows)).collect(),
        answers,
    };
    let mut asked: Vec<Asked> = job
        .plugins
        .iter()
        .map(|(plugin, merged)| Asked {
            plugin: plugin.clone(),
            merged: *merged,
            left: KINDS.len(),
            pages: Vec::new(),
            error: None,
            finished: false,
        })
        .collect();
    let mut pending: Vec<(usize, ItemKind, Reply<ItemPage>)> = Vec::new();
    for (source, (plugin, _)) in job.plugins.iter().enumerate() {
        for kind in KINDS {
            let reply = ctx.host.search(
                plugin,
                &job.query,
                SearchFilters {
                    kinds: vec![crate::remote::host_kind(kind)],
                },
                PageRequest {
                    cursor: None,
                    limit: PER_KIND,
                },
            );
            pending.push((source, kind, reply));
        }
    }
    let mut finished = 0;
    let deadline = Instant::now() + ctx.search_timeout;
    while !pending.is_empty() {
        let left = deadline.saturating_duration_since(Instant::now());
        let received = {
            let mut select = Select::new();
            select.recv(jobs);
            for (_, _, reply) in &pending {
                select.recv(reply);
            }
            match select.select_timeout(left) {
                Err(_) => None,
                Ok(operation) if operation.index() == 0 => {
                    // A newer search (or none: the core is shutting down).
                    return operation.recv(jobs).ok();
                }
                Ok(operation) => {
                    let index = operation.index() - 1;
                    Some((index, operation.recv(&pending[index].2)))
                }
            }
        };
        // Out of time: the sources that have not finished are published below.
        let Some((index, result)) = received else {
            break;
        };
        let (source, kind, _) = pending.remove(index);
        let entry = &mut asked[source];
        entry.left -= 1;
        match result {
            Ok(Ok(page)) => {
                ctx.remember(&entry.plugin, &page.items);
                entry.pages.push((kind, page));
            }
            Ok(Err(error)) => {
                ctx.note(&error);
                entry.error.get_or_insert_with(|| error.to_string());
            }
            Err(_) => {
                entry
                    .error
                    .get_or_insert_with(|| "the plugin host has shut down".to_owned());
            }
        }
        if entry.left == 0 {
            finished += 1;
            let done = finished == asked.len();
            publish(ctx, job, &mut asked[source], &mut merged, done);
        }
    }
    // Out of time: what came is published; a source with nothing is late.
    let late: Vec<usize> = (0..asked.len()).filter(|i| !asked[*i].finished).collect();
    for (n, source) in late.iter().enumerate() {
        let entry = &mut asked[*source];
        if entry.pages.is_empty() {
            entry.error = Some(format!("{} did not answer in time", ctx.name(&entry.plugin)));
        }
        publish(ctx, job, entry, &mut merged, n + 1 == late.len());
    }
    None
}

/// Publishes a source's section from the kinds it answered: its own,
/// or merged with the answers before it (and those of the earlier sections
/// that changed, just before). A source with no kind answered fails with its
/// first error.
fn publish(ctx: &Ctx, job: &SearchJob, asked: &mut Asked, merged: &mut Merged, done: bool) {
    asked.finished = true;
    let plugin = asked.plugin.clone();
    if asked.pages.is_empty() {
        let error = asked
            .error
            .take()
            .unwrap_or_else(|| format!("{} found nothing", ctx.name(&plugin)));
        ctx.emit_search(failed(job, &plugin, asked.merged, error, done));
        return;
    }
    // The tab's order; a plugin that ignores the kind asked for gives each
    // group only its kind.
    asked
        .pages
        .sort_by_key(|(kind, _)| KINDS.iter().position(|k| k == kind));
    let mut items = Vec::new();
    let mut cursors = Vec::new();
    for (kind, page) in asked.pages.drain(..) {
        items.extend(page.items.into_iter().filter(|i| convert::kind(i.kind) == kind));
        cursors.push((kind, page.next, page.total));
    }
    let answer = Answer::plugin(&plugin, items, &ctx.links(&plugin));
    let groups: Vec<SearchGroup> = cursors
        .into_iter()
        .filter_map(|(kind, next, total)| {
            let items: Vec<SourceItem> = answer.rows.iter().filter(|r| r.kind == kind).cloned().collect();
            (!items.is_empty()).then_some(SearchGroup {
                kind,
                items,
                next,
                total,
            })
        })
        .collect();
    if !asked.merged {
        // A section of its own: everything it found, nothing merged.
        ctx.emit_search(update(job, &plugin, answer.rows, false, groups, done));
        return;
    }
    merged.answers.push(answer);
    let mut sections = convert::sections(&merged.answers);
    let own = sections.pop().unwrap_or_default();
    for ((answer, (section, last)), groups) in merged
        .answers
        .iter()
        .zip(sections.into_iter().zip(merged.sent.iter_mut()))
        .zip(merged.sent_groups.iter())
    {
        if section != *last {
            let groups = groups.clone();
            ctx.emit_search(update(job, &answer.source, section.clone(), true, groups, false));
            *last = section;
        }
    }
    merged.sent.push(own.clone());
    merged.sent_groups.push(groups.clone());
    ctx.emit_search(update(job, &plugin, own, true, groups, done));
}

/// Rows kind by kind, in the order Songs, Albums, Artists, Playlists,
/// Folders (kinds not found are left out), without cursors: the library's
/// answer, which comes whole.
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
