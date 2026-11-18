# Putting Jellyfesh on GitHub

This repository is ready to push as it is: its workflows build, test and
publish everything once it is on GitHub. This page takes you from the
download pack (or any clone) to a repository whose **CI** checks every push,
whose **release workflow** builds and publishes each version, and whose
**website** (with Jellyfesh Web at `/app/`) is served by GitHub Pages.

| Workflow | Runs on | Does |
| --- | --- | --- |
| `ci.yml` | pushes and pull requests | Rust checks and tests on Linux, macOS and Windows, licences, the audio check, the bindings, and the macOS, Windows and Android app builds |
| `web.yml` | pushes and pull requests; on `main`, by hand, and after each release it also deploys | Jellyfesh Web's build and tests; the website on GitHub Pages |
| `release.yml` | a version tag (`v0.0.7`), or Run workflow | one installation file per platform and `SHA256SUMS.txt`; for a pushed tag (or with "Publish" ticked) a GitHub release (marked pre-release) with them and a Download table, otherwise one download on the run's page; the app stores' packages as run artifacts when their secrets are set |

## Before you push

1. **A GitHub account**, and git. The GitHub CLI (`gh`, signed in with
   `gh auth login`) lets the script below do every step for you.
2. **Public or private?** The licence is still "all rights reserved"
   (ADR-0016): a public repository lets everyone read the code, but not
   reuse it. On GitHub Free, **GitHub Pages works only for public
   repositories** (paid plans can publish Pages from a private one), and a
   private repository spends a monthly allowance of Actions minutes, which
   macOS and Windows runners use up faster; a release builds on both.
3. **The private keys archive** (`jellyfesh-v0.0.3-signing-keys-PRIVATE.zip`,
   kept apart from everything else, never in the repository). Unzip it
   somewhere outside the repository. The release workflow reads these
   repository secrets:

   | Secret | Value | Without it |
   | --- | --- | --- |
   | `JF_PLUGIN_SIGNING_KEY` | `plugin-release-key/jellyfesh-release-signing-key.hex` | Plugins ship unsigned and show as Community |
   | `JF_SPOTIFY_CLIENT_ID` | the Client ID of your Spotify app (see `docs/plugins/spotify.md`) | The Spotify plugin asks each person for their own Client ID |
   | `ANDROID_KEYSTORE_BASE64` | `android-release-keystore/ANDROID_KEYSTORE_BASE64.txt` | No APK in the release (its text says the APK is coming) |
   | `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_PASSWORD` | `android-release-keystore/keystore-password.txt` (both) | No APK |
   | `ANDROID_KEY_ALIAS` | `jellyfesh` | No APK |

   Selling Jellyfesh adds the secrets and variables of
   [Secrets and variables](#secrets-and-variables) (Apple, Windows, the
   Microsoft Store, Google Play and licence keys). Every one is optional: a
   release without it is still built and published, and the step that
   needed it says so in a notice.

## The one command

From the repository's folder (`jellyfesh/` in the download pack), with the
path to the unzipped keys folder:

```bash
tools/release/push-to-github.sh YOUR-NAME/jellyfesh --create public --keys PATH/TO/jellyfesh-signing-keys-PRIVATE
```

It makes the repository (`--create public` or `--create private`), sets the
five secrets from the key files (their values go straight to `gh` and are
never printed), turns on GitHub Pages with GitHub Actions as the source,
pushes `main`, and then pushes the newest version tag **on its own**, which
starts the release. Add `--dry-run` first to see every step without changing
anything, `--all-tags` to push the older tags too (in one push, for which
GitHub starts no workflow, so old releases are not rebuilt), `--ssh` to push
over SSH. `--help` lists the options.

## Without a terminal (GitHub Desktop and the website)

Everything below happens in [GitHub Desktop](https://desktop.github.com)
and on github.com; no command line.

1. **Use the repository folder itself.** Publish the `jellyfesh` folder of
   the download pack as it is (it holds the history in a hidden `.git`
   folder). Do not upload its files through the website or copy them into
   another folder with Finder or Explorer: both leave out the hidden folders
   (`.github` with the workflows, `.cargo`) and the scripts' permission to
   run, and without them GitHub runs nothing.
2. **If a repository with that name is already on GitHub** (from an upload),
   move it out of the way first: its Settings → General → Repository name →
   rename it (or delete it at the bottom of that page if nothing in it is
   needed).
3. **Publish:** in GitHub Desktop, File → Add Local Repository → choose the
   `jellyfesh` folder → Publish repository → name it `jellyfesh`, untick
   "Keep this code private" if the website should work on GitHub Free →
   Publish Repository. GitHub Desktop sends `main` and every tag. CI and Web
   start at once (Web skips its deploy, with a warning, until step 5).
4. **Secrets:** on github.com, the repository's Settings → Secrets and
   variables → Actions → New repository secret, once for each secret in the
   table above (paste each file's text; `ANDROID_KEY_ALIAS` is `jellyfesh`).
5. **Website:** Settings → Pages → Build and deployment → Source: **GitHub
   Actions**. Then Actions → **Web** → Run workflow.
6. **Release:** Actions → **Release** → Run workflow → type `v0.0.6` → Run
   workflow. (GitHub starts no workflow for the tags themselves, because
   they arrived together; this button builds the one you name. Its first
   job checks that the tag is on GitHub and says so if it is not.)

## By hand (without the GitHub CLI)

1. On github.com, create an empty repository (no README, licence or
   `.gitignore`).
2. Settings → Secrets and variables → Actions → New repository secret, for
   each secret in the table above (paste the file's contents).
3. Settings → Pages → Build and deployment → Source: **GitHub Actions**.
4. Push, `main` first and the release tag alone:

   ```bash
   git remote add origin https://github.com/YOUR-NAME/jellyfesh.git
   ```

   ```bash
   git push -u origin main
   ```

   ```bash
   git push origin v0.0.6
   ```

   Push the tag by itself: GitHub starts no workflow for a push of more than
   three tags, so `git push --tags` would build no release. If the tag is
   already on GitHub, start the release from Actions → Release → Run
   workflow instead, with the tag's name.

## What happens next

* **CI** starts on the push of `main`. The macOS, Windows and Android apps
  were never compiled before their first CI run (there is no Xcode, Windows
  or Android SDK where they were written): expect their jobs to need fixes.
* **The website**: `web.yml` builds Jellyfesh Web and deploys the site to
  `https://YOUR-NAME.github.io/jellyfesh/`, with the player at `/app/`.
* **The release**: `release.yml` builds every file (about an hour, mostly
  the macOS, Windows and Android jobs), publishes them as a pre-release
  whose text is a Download table and a link to `docs/releases/v0.0.7.md`
  ([What a release holds](#what-a-release-holds)), and starts `web.yml`
  again so the website lists the new files. It starts when the tag is pushed alone,
  or from Actions → Release → Run workflow with the tag's name. A job that
  fails leaves no release: fix it on `main`, delete the tag on GitHub, move
  it to the fixed commit and push it again (after a failure outside the
  code, such as a network error, just run the workflow again):

  ```bash
  git push origin --delete v0.0.6
  ```

  ```bash
  git tag -fa v0.0.6 -m "v0.0.6"
  ```

  ```bash
  git push origin v0.0.6
  ```

## What a release holds

One file for each platform, named after the version, and nothing else:

| File | What it is |
| --- | --- |
| `Jellyfesh-<version>-macOS.dmg` | The app for Apple silicon and Intel Macs; signed with the Developer ID and notarized when the Apple secrets are set |
| `Jellyfesh-<version>-Windows-Setup.exe` | One installer for x64 and Arm64 PCs (Inno Setup, `platforms/windows/installer/`): for the current user without administrator rights, with a Start menu shortcut and an uninstaller, over any earlier version; signed when the Windows secrets are set |
| `Jellyfesh-<version>-Linux.flatpak` | The Flatpak bundle: `flatpak install --user` fetches its GNOME runtime from Flathub (`flatpak build-bundle --runtime-repo`) |
| `Jellyfesh-<version>-Android.apk` | Signed with Jellyfesh's own release key. Without the Android secrets there is none, and the release's text says the APK is coming: it is never signed with another key |
| `Jellyfesh-<version>-Web.zip` | Jellyfesh Web to host yourself |
| `Jellyfesh-<version>-Community-Plugins.zip` | The unofficial NetEase Cloud Music and QQ Music plugins, which release builds do not bundle (ADR-0021 §4), with a README on installing them with Install from File… |
| `SHA256SUMS.txt` | The SHA-256 of each file above |

The release's text is a **Download** table, one row per platform with its
file, requirements and how to install it, then a link to
`docs/releases/vX.Y.Z.md` (`tools/release/release_body.py`). The website's
download page lists the same files: `web.yml` reads them, with their
checksums, from the latest release.

The apps carry their third-party notices, the official plugins come inside
every app, and the website is deployed by `web.yml`, so releases carry none
of them. The app stores' packages are not release files either: when their
secrets are set, a run keeps `Jellyfesh-<version>-Mac-App-Store.pkg`,
`-Microsoft-Store.msixbundle` and `-Google-Play.aab` on its page as the
artifacts `store-macos`, `store-windows` and `store-android`, to upload as
[`docs/store-release.md`](store-release.md) describes. The Windows portable
zips stay on the run's page for two weeks (`windows-portable-zips`), for
testing.

## Secrets and variables

Settings → Secrets and variables → Actions. A file's secret holds the file
in base64 (`base64 < FILE | gh secret set NAME`, or paste the output of
`base64 -i FILE` on a Mac).

| Name | Kind | Value | Without it |
| --- | --- | --- | --- |
| `JF_PLUGIN_SIGNING_KEY` | secret | the plugin release key (above) | Official plugins unsigned, shown as Community |
| `JF_SPOTIFY_CLIENT_ID` | secret | the Client ID of Jellyfesh's Spotify app | Each person enters their own Client ID |
| `ANDROID_KEYSTORE_BASE64`, `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS`, `ANDROID_KEY_PASSWORD` | secrets | Jellyfesh's Android release keystore (above); it also signs the Google Play bundle as its upload key | No APK (the release says it is coming) and no Google Play bundle |
| `PLAY_LICENSE_KEY` | secret or variable | Play Console → Monetize with Play → Monetization setup → Licensing: the app's base64 public key (not secret) | `BuildConfig.PLAY_LICENSE_KEY` is empty and the app cannot check the signatures of Play purchases |
| `APPLE_DEVELOPER_ID_P12`, `APPLE_DEVELOPER_ID_PASSWORD` | secrets | the Developer ID Application certificate with its private key (.p12, base64) and its password | The macOS app is ad-hoc signed and not notarized |
| `APPLE_TEAM_ID` | secret | the 10-character team ID | The identities of any team in the .p12 files are used |
| `APPLE_NOTARY_KEY_ID`, `APPLE_NOTARY_ISSUER_ID`, `APPLE_NOTARY_KEY` | secrets | an App Store Connect API key (Users and Access → Integrations → App Store Connect API, role Developer): its key ID, the issuer ID and the text of its `AuthKey_<id>.p8` | Not notarized |
| `APPLE_MAS_APP_P12`, `APPLE_MAS_INSTALLER_P12`, `APPLE_MAS_PASSWORD` | secrets | the Apple Distribution and Mac Installer Distribution certificates (.p12, base64) and their password | No Mac App Store package |
| `APPLE_MAS_PROFILE` | secret | the Mac App Store provisioning profile of `org.jellyfesh.Jellyfesh` (base64) | No Mac App Store package |
| `WINDOWS_SIGNING_PFX`, `WINDOWS_SIGNING_PASSWORD` | secrets | a code-signing certificate with its private key (.pfx, base64) and its password | The Windows app and installer are unsigned |
| `MS_STORE_IDENTITY_NAME`, `MS_STORE_PUBLISHER`, `MS_STORE_PUBLISHER_NAME` | secrets | Partner Center → the app → Product identity: Package/Identity/Name, Package/Identity/Publisher (`CN=…`) and Package/Properties/PublisherDisplayName | No Microsoft Store package |
| `JF_LICENCE_PUBLIC_KEYS` | variable | the public keys licence keys are checked with: `id:hex` entries separated by commas, without spaces, as `tools/jf-licence keygen` prints them (public keys are not secret) | No licence key is accepted; a malformed value stops the release at its first job |
| `JF_PRO_PREVIEW` | variable | `0` ends Pro Preview in the Jellyfesh Web the website serves (`web.yml`) | Pro Preview stays on there |

The private key of the licence keys (`tools/jf-licence keygen`) never goes
into the repository or a workflow; keep it in a password manager, or in the
secret `JF_LICENCE_SIGNING_KEY` if a workflow should ever issue keys (none
does yet).

Publicly trusted code-signing certificates issued since June 2023 keep
their private key in hardware (the CA/Browser Forum's baseline
requirements), so they cannot be exported as a .pfx: `WINDOWS_SIGNING_PFX`
suits a certificate held as a file. A hardware token or a cloud signing
service needs its own signing step ([`docs/store-release.md`](store-release.md)).

## Build settings: edition, distribution and Pro

These variables decide what a build is. The release workflow sets them; a
build without them is the community edition from source, without Pro
Preview and accepting no licence keys.

| Variable | Values | Set by | Read by |
| --- | --- | --- | --- |
| `JF_EDITION` | `community` (the default), `commercial` | `release.yml` for every job; `web.yml`'s deploy | `plugins/release.sh` (community packages go to `dist/community-plugins/`), every app's build (no `community.unofficial.*` bundled), and the apps |
| `JF_DISTRIBUTION` | `source` (the default), `direct`, `app-store`, `microsoft-store`, `google-play`, `web` | `release.yml` for each app; the store builds set their own | the apps, which give it to the core (`CoreConfig.distribution`) |
| `JF_PRO_PREVIEW` | `1` turns Pro Preview on; anything else, or nothing, leaves it off | `release.yml`'s **Pro Preview** box (always on for a pushed tag); `web.yml` from the variable `JF_PRO_PREVIEW` | every Rust build (the core), with `option_env!` |
| `JF_LICENCE_PUBLIC_KEYS` | `id:hex,id:hex` | the variable of the same name | every Rust build (`core/jf-licence`) |
| `PLAY_LICENSE_KEY` | Play's base64 public key | the secret or variable of the same name | the Android build (`BuildConfig.PLAY_LICENSE_KEY`) |
| `GGML_NATIVE` | `OFF` | `release.yml`, the build scripts, the Flatpak manifest | whisper.cpp's build: code for every CPU of the architecture, never the build machine's alone |

How each app sees the edition and the distribution:

| App | Edition and distribution | Distributions |
| --- | --- | --- |
| macOS | the Info.plist keys `JFEdition` and `JFDistribution`, and the Swift conditions `JF_EDITION_COMMUNITY` or `JF_EDITION_COMMERCIAL` and `JF_DISTRIBUTION_SOURCE`, `_DIRECT` or `_APP_STORE` (`project.yml`; `scripts/build.sh` sets them from the environment) | `source`, `direct`, `app-store` |
| Windows | the constants `JF_EDITION_*` and `JF_DISTRIBUTION_*` (for example `JF_DISTRIBUTION_MICROSOFT_STORE`) and the assembly metadata `JfEdition` and `JfDistribution` (`Directory.Build.props` and `.targets`; MSBuild properties `JfEdition` and `JfDistribution`, else the environment) | `source`, `direct`, `microsoft-store` (`build.ps1 -StorePackage`) |
| Android | `BuildConfig.EDITION` and `BuildConfig.DISTRIBUTION` (the Gradle properties `jellyfesh.edition` and `jellyfesh.distribution`, else the environment) | `source`, `direct` (the APK), `google-play` (`build.sh --play-bundle`) |
| Linux | `option_env!("JF_EDITION")` and `option_env!("JF_DISTRIBUTION")` in the app's Rust code (the Flatpak build gets them through its manifest) | `source`, `direct` |
| Jellyfesh Web | `import.meta.env.VITE_JF_EDITION` and `import.meta.env.VITE_JF_DISTRIBUTION` (Vite passes on the build environment's `VITE_*` variables) | `web` |

## Signing a release after the fact

Without the `JF_PLUGIN_SIGNING_KEY` secret, a release's official plugins are
unsigned: every app shows them as Community plugins and Add a Source lists
no services. To fix a published release, add the secret (Secrets, above),
then Actions → **Release** → Run workflow → type its tag (for example
`v0.0.7`), tick **Publish a GitHub release** → Run workflow. It builds every
file again, now with signed plugins, and replaces the release's files and
text. Likewise for the Android keystore secrets, without which a release has
no APK, and for the Apple and Windows signing secrets. Leave **Pro Preview**
as the release was built.

## The installation files without a release

Actions → **Release** → Run workflow, leave **tag** empty and **Publish a
GitHub release** unticked, choose the branch (normally `main`) and run it.
Every app is built from that branch and nothing is published: when the run
ends, its page has one download under **Artifacts**,
`Jellyfesh-v0.0.7-main-<commit>`, with the release's files (named
`Jellyfesh-0.0.7-main-<commit>-macOS.dmg` and so on) and `SHA256SUMS.txt`,
next to the store packages the secrets allowed. It is there for 30 days, and
it holds whatever was built even when some platforms failed (their jobs show
red). Type a tag such as `v0.0.7` instead to build that version without
publishing it.

## Later releases

Raise the version in `Cargo.toml` (`[workspace.package]`),
`platforms/web/package.json`, the Linux metainfo,
`platforms/macos/project.yml` (`MARKETING_VERSION`, and
`CURRENT_PROJECT_VERSION` one higher), `platforms/windows/Directory.Build.props`
and `platforms/android/app/build.gradle.kts` (`versionName`, and
`versionCode` one higher: the stores refuse a build number they have seen),
write `docs/releases/vX.Y.Z.md` and the `CHANGELOG.md` entry, commit, tag
(`git tag -a vX.Y.Z -m "..."`) and push the tag alone.
