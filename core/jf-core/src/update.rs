//! Update checks for direct downloads (ADR-0021 §5).
//!
//! Only builds with `Distribution::Direct` check: at most once a day while
//! `Settings::check_for_updates` is on, and on `Command::CheckForUpdates`.
//! The check reads GitHub's list of releases (`Config::update_url`) and
//! offers the newest release newer than this build, pre-releases included
//! while Jellyfesh is in beta. Nothing is downloaded or installed. The
//! request carries no identifier beyond what any HTTPS request does.
//!
//! A thread of its own does the waiting and the asking, and publishes the
//! `Update` slice itself. When the last check was is kept in
//! `<data>/update.json`, so a restart does not check again within the day.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender, unbounded};
use serde::{Deserialize, Serialize};

use crate::actor::Shared;
use crate::types::{Event, UpdateInfo, UpdateSnapshot};

/// The shortest time between automatic checks.
pub(crate) const INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
/// The most of the releases list read.
const MAX_BODY: u64 = 4 * 1024 * 1024;

/// What the actor tells the update thread.
#[derive(Debug)]
pub(crate) enum UpdateMsg {
    /// Check now (`Command::CheckForUpdates`).
    CheckNow,
    /// The setting changed.
    Auto(bool),
}

/// What is kept between runs.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Saved {
    last_checked_ms: Option<i64>,
    available: Option<(String, String)>,
}

/// One GitHub release, as the list gives it.
#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    draft: bool,
}

/// The newest release in `body` (GitHub's JSON list) that is newer than
/// `current`, pre-releases included. Tags may start with `v`; releases
/// whose tag is not a version, and drafts, are skipped.
pub(crate) fn newest(body: &[u8], current: &semver::Version) -> Result<Option<UpdateInfo>, String> {
    let releases: Vec<Release> =
        serde_json::from_slice(body).map_err(|e| format!("the release list could not be read: {e}"))?;
    let best = releases
        .iter()
        .filter(|r| !r.draft)
        .filter_map(|r| {
            let version = semver::Version::parse(r.tag_name.trim().trim_start_matches(['v', 'V'])).ok()?;
            Some((version, r))
        })
        .filter(|(version, _)| version > current)
        .max_by(|a, b| a.0.cmp(&b.0));
    Ok(best.map(|(version, release)| UpdateInfo {
        version: version.to_string(),
        url: release.html_url.clone(),
    }))
}

/// This build's version.
pub(crate) fn current_version() -> semver::Version {
    semver::Version::parse(env!("CARGO_PKG_VERSION")).unwrap_or_else(|_| semver::Version::new(0, 0, 0))
}

/// Asks `url` for the releases.
fn fetch(url: &str) -> Result<Vec<u8>, String> {
    let mut response = crate::sources::http_agent()
        .get(url)
        .header("accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("could not reach the release list: {e}"))?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(format!("the release list answered {status}"));
    }
    response
        .body_mut()
        .with_config()
        .limit(MAX_BODY)
        .read_to_vec()
        .map_err(|e| format!("reading the release list: {e}"))
}

/// The update thread's state.
struct Checker {
    shared: Arc<Shared>,
    url: String,
    file: PathBuf,
    supported: bool,
    auto: bool,
    saved: Saved,
    version: u64,
    error: Option<String>,
}

impl Checker {
    fn snapshot(&self, checking: bool) -> UpdateSnapshot {
        UpdateSnapshot {
            version: self.version,
            supported: self.supported,
            checking,
            available: self.saved.available.as_ref().map(|(version, url)| UpdateInfo {
                version: version.clone(),
                url: url.clone(),
            }),
            last_checked_ms: self.saved.last_checked_ms,
            auto: self.supported && self.auto,
            error: self.error.clone(),
        }
    }

    fn publish(&mut self, checking: bool) {
        self.version += 1;
        let snapshot = Arc::new(self.snapshot(checking));
        self.shared.update(|s| s.update = snapshot.clone());
        self.shared.emit(Event::Update(snapshot));
    }

    fn check(&mut self) {
        self.publish(true);
        let result = fetch(&self.url).and_then(|body| newest(&body, &current_version()));
        let now = crate::actor::unix_ms();
        match result {
            Ok(available) => {
                self.saved.available = available.map(|u| (u.version, u.url));
                self.saved.last_checked_ms = Some(now);
                self.error = None;
            }
            Err(error) => {
                tracing::info!(%error, "the update check failed");
                self.error = Some(error);
                // A failed check counts too, so a server that is down is not asked every minute.
                self.saved.last_checked_ms = Some(now);
            }
        }
        if let Ok(json) = serde_json::to_vec(&self.saved)
            && let Err(e) = crate::sources::write_atomic(&self.file, &json)
        {
            tracing::warn!("could not save the update check: {e}");
        }
        self.publish(false);
    }

    /// How long until the next automatic check is due.
    fn due_in(&self) -> Option<Duration> {
        if !(self.supported && self.auto) {
            return None;
        }
        let last = self.saved.last_checked_ms?;
        let next = last.saturating_add(INTERVAL.as_millis() as i64);
        let left = next.saturating_sub(crate::actor::unix_ms()).max(0);
        Some(Duration::from_millis(left as u64))
    }

    fn run(mut self, rx: &Receiver<UpdateMsg>) {
        self.publish(false);
        loop {
            let wait = match (self.supported && self.auto, self.saved.last_checked_ms) {
                (false, _) => None,
                (true, None) => Some(Duration::ZERO),
                (true, Some(_)) => self.due_in(),
            };
            let message = match wait {
                Some(wait) => match rx.recv_timeout(wait) {
                    Ok(message) => Some(message),
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => None,
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => return,
                },
                None => match rx.recv() {
                    Ok(message) => Some(message),
                    Err(_) => return,
                },
            };
            match message {
                None => self.check(),
                Some(UpdateMsg::CheckNow) => {
                    if self.supported {
                        self.check();
                    }
                }
                Some(UpdateMsg::Auto(on)) => {
                    if on != self.auto {
                        self.auto = on;
                        self.publish(false);
                    }
                }
            }
        }
    }
}

/// Starts the update thread; it publishes the `Update` slice at once.
pub(crate) fn spawn(
    shared: Arc<Shared>,
    url: String,
    data_dir: &std::path::Path,
    supported: bool,
    auto: bool,
) -> std::io::Result<Sender<UpdateMsg>> {
    let (tx, rx) = unbounded();
    let file = data_dir.join("update.json");
    let saved: Saved = std::fs::read(&file)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    let checker = Checker {
        shared,
        url,
        file,
        supported,
        auto,
        saved,
        version: 0,
        error: None,
    };
    std::thread::Builder::new()
        .name("jf-update".into())
        .spawn(move || checker.run(&rx))?;
    Ok(tx)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(tags: &[(&str, bool)]) -> Vec<u8> {
        let releases: Vec<serde_json::Value> = tags
            .iter()
            .map(|(tag, draft)| {
                serde_json::json!({
                    "tag_name": tag,
                    "html_url": format!("https://github.com/ZodiaCALs/Jellyfesh/releases/tag/{tag}"),
                    "draft": draft,
                    "prerelease": tag.contains('-'),
                })
            })
            .collect();
        serde_json::to_vec(&releases).unwrap()
    }

    #[test]
    fn picks_the_newest_newer_release_pre_releases_included() {
        let current = semver::Version::parse("0.0.7").unwrap();
        let body = list(&[
            ("v0.0.6", false),
            ("v0.0.8-beta.1", false),
            ("v0.0.7", false),
            ("nightly", false),
        ]);
        let found = newest(&body, &current).unwrap().unwrap();
        assert_eq!(found.version, "0.0.8-beta.1");
        assert!(found.url.ends_with("/v0.0.8-beta.1"));
        // A release beats its own pre-releases; drafts never count.
        let body = list(&[("v0.0.8-beta.1", false), ("v0.0.8", false), ("v0.1.0", true)]);
        assert_eq!(newest(&body, &current).unwrap().unwrap().version, "0.0.8");
        // Nothing newer, or only older releases.
        assert_eq!(
            newest(&list(&[("v0.0.7", false), ("0.0.5", false)]), &current).unwrap(),
            None
        );
        // A pre-release of this version is older than it.
        assert_eq!(newest(&list(&[("v0.0.7-rc.1", false)]), &current).unwrap(), None);
        // A pre-release build sees its release as newer.
        let beta = semver::Version::parse("0.0.7-beta.2").unwrap();
        assert_eq!(
            newest(&list(&[("v0.0.7", false)]), &beta)
                .unwrap()
                .unwrap()
                .version,
            "0.0.7"
        );
        assert!(newest(b"not json", &current).is_err());
        assert!(newest(b"{}", &current).is_err());
    }
}
