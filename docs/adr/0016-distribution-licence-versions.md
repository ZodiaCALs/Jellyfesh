# ADR-0016: Distribution, licence and versions

* Status: Accepted (your answers of 2026-11-05)
* Related: ADR-0012 (community plugins), ADR-0010/0011 (deferred vendors)

## Context

Your answers: distribute from Jellyfesh's own website and GitHub releases
first; make a commercial version once the features are complete (you will
say when); no licence for now; skip developer accounts (Apple, Spotify,
token service) until the commercial version; and make every version of the
repository downloadable.

## Decision

### Channels and builds

No app stores for now. Each release is published on GitHub Releases and
linked from the website.

| Platform | Release files | Signing now (no paid accounts) | First-run experience |
| --- | --- | --- | --- |
| macOS 14.2+ | `.dmg` with a universal (arm64 + x86_64) `.app` | Ad-hoc signature only; not notarised | macOS blocks the first launch; the person allows it in System Settings → Privacy & Security → Open Anyway (since macOS 15 the Control-click bypass no longer exists) |
| Windows 10 1809+ | Installer `.exe` and portable `.zip`, x64 and arm64; Windows App SDK self-contained, so no runtime install | Unsigned | SmartScreen: "More info → Run anyway" until the publisher builds reputation |
| Linux | `.flatpak` bundle on the GNOME 51 runtime from Flathub | Unsigned bundle | `flatpak install --bundle`; Flathub listing later |
| Android 8.0+ | `.apk` per ABI (arm64-v8a, armeabi-v7a, x86_64) and a universal APK | Our own release key (free; keep it backed up, it can never change) | "Install unknown apps" permission |

When the commercial version starts: Apple Developer ID and notarisation,
Windows code signing, Flathub, Google Play and other stores, plus the
store-policy rules in ADR-0012 (they apply only to store builds).

Because notarisation and MusicKit both need the Apple Developer Program,
Apple Music (ADR-0010) and Spotify (ADR-0011) move to the commercial
phase too.

### Updates

The app checks the website's release feed (a JSON mirror of the latest
GitHub release) at most once a day. The check is on by default, explained on
first run and can be turned off. Updates are downloaded by the person
through the browser; there is no silent self-update while builds are
unsigned, because an unsigned self-updater is an easy target.

### Licence

No licence file for now, which means **all rights reserved**. To keep a
closed commercial version possible, dependencies are limited to:

* allowed: MIT, Apache-2.0, BSD-2/3-Clause, ISC, Zlib, CC0-1.0,
  Unicode-3.0, MPL-2.0 (file-level copyleft: changes to MPL files are
  published), LGPL only when dynamically linked (GTK, libadwaita,
  GStreamer on Linux);
* denied: GPL, AGPL, SSPL, and code with no licence.

`cargo-deny`, and the Gradle and NuGet licence reports, fail CI on a denied
licence. Community plugins live in their own repository and choose their own
licences.

If the source repository is public without a licence, people can read the
code but have no right to reuse it. If you want the source private and the
releases public, keep the source in a private repository and publish
binaries from a separate public `jellyfesh-releases` repository. ⚑ Your call
when we create the GitHub repositories.

### Versions

* App versions follow SemVer, `0.x` until 1.0.
* Repository versions are annotated git tags: `step-0.N` for planning
  revisions, then `v0.M.P`, where `M` is the milestone whose gate the tag
  passed (for example `v0.1.0` at the Milestone 1 gate).
* **Every delivered version comes with downloadable archives**:
  `jellyfesh-<tag>.zip` (a snapshot of the files) and
  `jellyfesh-<tag>.bundle` (the full history; `git clone
  jellyfesh-<tag>.bundle jellyfesh`). `CHANGELOG.md` lists each version.

## Consequences

* First-run warnings on macOS and Windows until the commercial phase adds
  signing; the website's download page shows the steps with screenshots.
* App Store policy risks (R4) are dormant until store builds exist.
* The Android release key is a long-lived secret we must store safely from
  the first APK on.
