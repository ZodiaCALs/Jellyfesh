#!/usr/bin/env bash
# Installs Jellyfesh into a prefix: the binary, the desktop entry, the
# AppStream metadata, the icons and the translations. The Flatpak build and
# the tarball both use it.
#
#   install.sh --binary target/release/jellyfesh [--prefix /usr/local] [--destdir DIR]
#
# Needs msgfmt (gettext) for the translations; without it the app is
# installed in English only. At run time the app needs GTK 4.14 or later
# and libadwaita 1.5 or later.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
app="$here/.."
prefix=/usr/local
destdir=
binary=
while [[ $# -gt 0 ]]; do
  case "$1" in
    --prefix) prefix=$2; shift 2 ;;
    --destdir) destdir=$2; shift 2 ;;
    --binary) binary=$2; shift 2 ;;
    -h|--help) sed -n '2,11p' "$0"; exit 0 ;;
    *) echo "unknown option $1" >&2; exit 2 ;;
  esac
done
[[ -n "$binary" && -x "$binary" ]] || { echo "pass --binary path/to/jellyfesh" >&2; exit 2; }

root="$destdir$prefix"
id=org.jellyfesh.Jellyfesh
install -Dm755 "$binary" "$root/bin/jellyfesh"
install -Dm644 "$app/data/$id.desktop" "$root/share/applications/$id.desktop"
install -Dm644 "$app/data/$id.metainfo.xml" "$root/share/metainfo/$id.metainfo.xml"
install -Dm644 "$app/data/icons/hicolor/scalable/apps/$id.svg" "$root/share/icons/hicolor/scalable/apps/$id.svg"
install -Dm644 "$app/data/icons/hicolor/symbolic/apps/$id-symbolic.svg" \
  "$root/share/icons/hicolor/symbolic/apps/$id-symbolic.svg"
if command -v msgfmt >/dev/null; then
  for lang in $(grep -v '^#' "$app/po/LINGUAS"); do
    install -d "$root/share/locale/$lang/LC_MESSAGES"
    msgfmt -o "$root/share/locale/$lang/LC_MESSAGES/jellyfesh.mo" "$app/po/$lang.po"
  done
else
  echo "msgfmt not found: installing without translations" >&2
fi
# The official plugins, when the release build made them (plugins/release.sh):
# the core installs them on first start and updates them with the app.
repo="$(cd "$app/../.." && pwd)"
shopt -s nullglob
plugins=("$repo"/dist/plugins/*.jfpkg)
if (( ${#plugins[@]} )); then
  install -d "$root/share/jellyfesh/plugins"
  install -m644 "${plugins[@]}" "$root/share/jellyfesh/plugins/"
else
  echo "no dist/plugins/*.jfpkg: installing without the bundled plugins" >&2
fi
if [[ -f "$repo/THIRD_PARTY_NOTICES.md" ]]; then
  install -Dm644 "$repo/THIRD_PARTY_NOTICES.md" "$root/share/doc/jellyfesh/THIRD_PARTY_NOTICES.md"
fi
echo "installed Jellyfesh into $root"
