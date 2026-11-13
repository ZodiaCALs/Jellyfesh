# Jellyfesh (working name)

An elegant, very lightweight music player for macOS, Windows, Linux,
Android and the web. It plays local files, and online sources through
sandboxed plugins.

**Status: v0.0.4 "Open Water".** Native apps for Linux (built and run),
macOS, Windows and Android (complete in code, first compiled by CI) and
**Jellyfesh Web**, the player in the browser for Subsonic, Navidrome and
Jellyfin servers ([`docs/web.md`](docs/web.md)), over one Rust core; online
sources through sandboxed plugins, with official Subsonic/OpenSubsonic and
Jellyfin plugins; playlists, favourites, play history, crossfade and output
switching; a design after Apple Music and Media Player in the Misty language,
with a straight progress line, a player bar for wide windows, and accent,
transparency and lyrics settings. Release notes:
[`docs/releases/v0.0.4.md`](docs/releases/v0.0.4.md). Design decisions:
[`docs/step-0/README.md`](docs/step-0/README.md). Version history:
[`CHANGELOG.md`](CHANGELOG.md). To put the repository on GitHub, with CI,
releases and the website: [`docs/github.md`](docs/github.md).

```bash
cargo run --release -p jellyfesh-gtk                       # the Linux app (GTK 4.14+, libadwaita 1.5+)
cargo run --release -p jf-cli -- scan ~/Music              # the command-line player
cargo run --release -p jf-cli -- play "an album title" --lyrics
```

Each app's build notes: [`docs/macos.md`](docs/macos.md),
[`docs/windows.md`](docs/windows.md), [`docs/android.md`](docs/android.md),
[`docs/web.md`](docs/web.md) (Jellyfesh Web); the Linux app's packaging is in
[`platforms/linux/packaging/`](platforms/linux/packaging/).

No licence has been chosen yet: all rights reserved (see ADR-0016).

| Area | Where |
| --- | --- |
| Release notes | [`docs/releases/`](docs/releases/) |
| Publishing on GitHub (CI, releases, website) | [`docs/github.md`](docs/github.md), [`tools/release/push-to-github.sh`](tools/release/push-to-github.sh) |
| How to test | [`docs/testing.md`](docs/testing.md) |
| Rust core and CLI | [`core/`](core/), [`apps/cli/`](apps/cli/) |
| The core API every app uses | [`docs/core-api.md`](docs/core-api.md), [`docs/ffi.md`](docs/ffi.md) |
| Apps | [`platforms/`](platforms/) (linux, macos, windows, android, web, shared) |
| Jellyfesh Web (browser) | [`platforms/web/`](platforms/web/), [`core/jf-web/`](core/jf-web/), [`docs/web.md`](docs/web.md) |
| Plugins: SDK, official plugins, authoring | [`plugins/`](plugins/), [`tools/jf-plugin/`](tools/jf-plugin/), [`docs/plugins/`](docs/plugins/) |
| Step 0 review package (start here) | [`docs/step-0/README.md`](docs/step-0/README.md) |
| Architecture overview | [`docs/architecture.md`](docs/architecture.md) |
| Decision records | [`docs/adr/`](docs/adr/) |
| Repository layout | [`docs/repo-layout.md`](docs/repo-layout.md) |
| Risks | [`docs/risks.md`](docs/risks.md) |
| Milestones | [`docs/milestones.md`](docs/milestones.md) |
| Online sources: API and terms findings | [`docs/sources-report.md`](docs/sources-report.md) |
| Design tokens | [`design/tokens.json`](design/tokens.json), [`docs/design-tokens.md`](docs/design-tokens.md) |
| Control styles (options) | [`docs/design-controls.md`](docs/design-controls.md) |
| App icon concepts | [`design/icons/`](design/icons/) |
| Wordmark and lockups | [`design/brand/`](design/brand/) |
