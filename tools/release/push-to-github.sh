#!/usr/bin/env bash
# Puts this repository on GitHub, ready for its workflows (docs/github.md).
#
#   tools/release/push-to-github.sh OWNER/REPO [options]
#
#   --create public|private  make the repository first (GitHub CLI, `gh`)
#   --keys DIR               set the release secrets from the unpacked private
#                            keys archive (jellyfesh-signing-keys-PRIVATE/)
#   --ssh                    push over SSH (git@github.com:OWNER/REPO.git)
#                            instead of HTTPS
#   --all-tags               also push the older tags (in one push of more
#                            than three tags, for which GitHub starts no
#                            workflow, so old releases are not rebuilt)
#   --no-tag                 push main only, no release
#   --yes                    do not ask before pushing
#   --dry-run                show what would happen; change nothing on GitHub
#
# Steps: the `origin` remote; with --create, the repository; with --keys, the
# secrets release.yml reads (values go from the files to `gh` without being
# printed); GitHub Pages with GitHub Actions as its source (with `gh`); then
# main (CI and the website deploy run) and the newest version tag on its own
# (release.yml builds and publishes that release).
set -euo pipefail

usage() { sed -n '2,17p' "$0" | sed 's/^# \{0,1\}//'; exit "${1:-0}"; }
say() { printf '\n== %s\n' "$*"; }
die() { printf 'push-to-github: %s\n' "$*" >&2; exit 1; }

repo="" create="" keys="" ssh=0 all_tags=0 no_tag=0 yes=0 dry=0
while [ $# -gt 0 ]; do
  case "$1" in
    -h|--help) usage 0 ;;
    --create) create="${2:-}"; shift 2 ;;
    --keys) keys="${2:-}"; shift 2 ;;
    --ssh) ssh=1; shift ;;
    --all-tags) all_tags=1; shift ;;
    --no-tag) no_tag=1; shift ;;
    --yes) yes=1; shift ;;
    --dry-run) dry=1; shift ;;
    -*) die "unknown option $1 (see --help)" ;;
    *) [ -z "$repo" ] || die "one OWNER/REPO only"; repo="$1"; shift ;;
  esac
done
[ -n "$repo" ] || usage 1
case "$repo" in
  https://github.com/*) repo="${repo#https://github.com/}"; repo="${repo%.git}" ;;
  git@github.com:*) repo="${repo#git@github.com:}"; repo="${repo%.git}" ;;
esac
case "$repo" in */*) ;; *) die "give the repository as OWNER/REPO, for example you/jellyfesh" ;; esac
case "$create" in ""|public|private) ;; *) die "--create takes public or private" ;; esac

# ---- the checkout ----------------------------------------------------------------------------
root=$(git rev-parse --show-toplevel 2>/dev/null) || die "run this inside the Jellyfesh repository"
cd "$root"
[ -f Cargo.toml ] && [ -d .github/workflows ] || die "$root does not look like the Jellyfesh repository"
branch=$(git symbolic-ref --short HEAD 2>/dev/null || true)
[ "$branch" = main ] || die "check out main first (on '${branch:-a detached HEAD}')"
[ -z "$(git status --porcelain --untracked-files=no)" ] || die "commit or stash your changes first"
newest=$(git tag -l 'v*' --sort=-v:refname | head -n 1)
if [ "$no_tag" -eq 0 ]; then
  [ -n "$newest" ] || die "no version tag (vX.Y.Z) to release; use --no-tag to push main only"
  git merge-base --is-ancestor "$newest" main || die "$newest is not on main"
  [ -f "docs/releases/$newest.md" ] || die "docs/releases/$newest.md is missing (release.yml uses it)"
fi

have_gh=0
if command -v gh >/dev/null 2>&1 && gh auth status >/dev/null 2>&1; then have_gh=1; fi
if [ "$have_gh" -eq 0 ] && [ "$dry" -eq 0 ] && { [ -n "$create" ] || [ -n "$keys" ]; }; then
  die "--create and --keys need the GitHub CLI, installed and signed in (gh auth login); or do those steps on github.com (docs/github.md)"
fi

if [ "$ssh" -eq 1 ]; then url="git@github.com:$repo.git"; else url="https://github.com/$repo.git"; fi
# Runs a command that changes something, or with --dry-run shows it.
run() { if [ "$dry" -eq 1 ]; then echo "would run: $*"; else "$@"; fi; }

say "Repository $repo ($url)"
if [ -n "$create" ]; then
  if [ "$have_gh" -eq 1 ] && gh repo view "$repo" >/dev/null 2>&1; then
    echo "It exists already; using it."
  else
    run gh repo create "$repo" "--$create" --description "Jellyfesh, an elegant, lightweight music player"
  fi
fi
if git remote get-url origin >/dev/null 2>&1; then
  run git remote set-url origin "$url"
else
  run git remote add origin "$url"
fi
echo "origin -> $url"

# ---- secrets ---------------------------------------------------------------------------------
if [ -n "$keys" ]; then
  say "Release secrets from $keys"
  plugin="$keys/plugin-release-key/jellyfesh-release-signing-key.hex"
  android="$keys/android-release-keystore"
  [ -f "$plugin" ] || die "$plugin not found (unzip jellyfesh-*-signing-keys-PRIVATE.zip and pass its folder)"
  set_secret() { # NAME FILE: the file's text without line breaks, from stdin, never echoed
    if [ "$dry" -eq 1 ]; then echo "would set $1 from $2"; return; fi
    tr -d '\r\n' < "$2" | gh secret set "$1" --repo "$repo"
    echo "set $1"
  }
  set_secret JF_PLUGIN_SIGNING_KEY "$plugin"
  if [ -f "$android/ANDROID_KEYSTORE_BASE64.txt" ] && [ -f "$android/keystore-password.txt" ]; then
    set_secret ANDROID_KEYSTORE_BASE64 "$android/ANDROID_KEYSTORE_BASE64.txt"
    set_secret ANDROID_KEYSTORE_PASSWORD "$android/keystore-password.txt"
    set_secret ANDROID_KEY_PASSWORD "$android/keystore-password.txt"
    run gh secret set ANDROID_KEY_ALIAS --repo "$repo" --body jellyfesh
    [ "$dry" -eq 1 ] || echo "set ANDROID_KEY_ALIAS"
  else
    echo "No Android keystore in $android: the release will have no APK."
  fi
fi

# ---- GitHub Pages ----------------------------------------------------------------------------
if [ "$have_gh" -eq 1 ] && [ "$dry" -eq 1 ]; then
  say "GitHub Pages (source: GitHub Actions)"
  echo "would run: gh api --method POST repos/$repo/pages -f build_type=workflow"
elif [ "$have_gh" -eq 1 ]; then
  say "GitHub Pages (source: GitHub Actions)"
  if gh api --method POST "repos/$repo/pages" -f build_type=workflow >/dev/null 2>&1 \
    || gh api --method PUT "repos/$repo/pages" -f build_type=workflow >/dev/null 2>&1; then
    echo "on"
  else
    echo "Could not turn Pages on (on GitHub Free it needs a public repository)."
    echo "Settings -> Pages -> Source: GitHub Actions, or the website deploy in web.yml will fail."
  fi
else
  say "Without the GitHub CLI, do these on github.com before the push (docs/github.md):"
  echo "  * Settings -> Pages -> Source: GitHub Actions"
  echo "  * Settings -> Secrets and variables -> Actions: JF_PLUGIN_SIGNING_KEY and the ANDROID_* secrets"
fi

# ---- push ------------------------------------------------------------------------------------
older=()
if [ "$all_tags" -eq 1 ]; then
  while IFS= read -r t; do [ "$t" = "$newest" ] || older+=("$t"); done < <(git tag -l)
fi
say "About to push"
echo "  main ($(git rev-parse --short main))"
if [ "${#older[@]}" -gt 0 ]; then echo "  older tags: ${older[*]}"; fi
if [ "$no_tag" -eq 0 ]; then echo "  $newest, alone, which starts the release workflow"; fi
if [ "$yes" -eq 0 ] && [ "$dry" -eq 0 ]; then
  printf 'Push now? [y/N] '
  read -r answer
  case "$answer" in y|Y|yes|YES) ;; *) die "nothing pushed" ;; esac
fi
run git push -u origin main
if [ "${#older[@]}" -gt 0 ]; then
  if [ "${#older[@]}" -le 3 ]; then
    echo "Note: a push of three tags or fewer starts the release workflow for each version tag in it."
  fi
  run git push origin "${older[@]}"
fi
if [ "$no_tag" -eq 0 ]; then run git push origin "$newest"; fi

if [ "$dry" -eq 1 ]; then say "Dry run: nothing was changed"; else say "Done"; fi
echo "Actions:  https://github.com/$repo/actions"
if [ "$no_tag" -eq 0 ]; then echo "Release:  https://github.com/$repo/releases/tag/$newest (when release.yml finishes)"; fi
owner="${repo%%/*}" name="${repo#*/}"
echo "Website:  https://$owner.github.io/$name/ (Jellyfesh Web at /app/), when web.yml finishes"
