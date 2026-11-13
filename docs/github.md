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
| `release.yml` | a version tag (`v0.0.4`), or Run workflow | every installation file; for a pushed tag (or with "Publish" ticked) a GitHub release (marked pre-release) with them and the website for that release, otherwise one download on the run's page |

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
   | `ANDROID_KEYSTORE_BASE64` | `android-release-keystore/ANDROID_KEYSTORE_BASE64.txt` | No APK in the release |
   | `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_PASSWORD` | `android-release-keystore/keystore-password.txt` (both) | No APK |
   | `ANDROID_KEY_ALIAS` | `jellyfesh` | No APK |

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
6. **Release:** Actions → **Release** → Run workflow → type `v0.0.4` → Run
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
   git push origin v0.0.4
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
  with `docs/releases/v0.0.4.md` as its text, and starts `web.yml` again so
  the website lists the new files. It starts when the tag is pushed alone,
  or from Actions → Release → Run workflow with the tag's name. A job that
  fails leaves no release: fix it on `main`, delete the tag on GitHub, move
  it to the fixed commit and push it again (after a failure outside the
  code, such as a network error, just run the workflow again):

  ```bash
  git push origin --delete v0.0.4
  ```

  ```bash
  git tag -fa v0.0.4 -m "v0.0.4"
  ```

  ```bash
  git push origin v0.0.4
  ```

## The installation files without a release

Actions → **Release** → Run workflow, leave **tag** empty and **Publish a
GitHub release** unticked, choose the branch (normally `main`) and run it.
Every app is built from that branch and nothing is published: when the run
ends, its page has one download under **Artifacts**,
`Jellyfesh-v0.0.4-main-<commit>`, with jf, the plugins, the Linux tarball and
Flatpak, the macOS DMG, the Windows zips, the Android APK and Jellyfesh Web.
It is there for 30 days, and it holds whatever was built even when some
platforms failed (their jobs show red). Type a tag such as `v0.0.4` instead
to build that version without publishing it.

## Later releases

Raise the version in `Cargo.toml` (`[workspace.package]`),
`platforms/web/package.json` and the Linux metainfo, write
`docs/releases/vX.Y.Z.md` and the `CHANGELOG.md` entry, commit, tag
(`git tag -a vX.Y.Z -m "..."`) and push the tag alone.
