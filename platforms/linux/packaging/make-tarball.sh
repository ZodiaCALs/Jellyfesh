#!/usr/bin/env bash
# Builds a tarball for distributions with GTK 4.14+ and libadwaita 1.5+
# (Ubuntu 24.04 and later, Debian 13, Fedora 40 and later):
#
#   jellyfesh-<version>-<arch>/
#     bin/jellyfesh
#     share/{applications,metainfo,icons,locale}/…
#     install.sh        copies the above into a prefix (default ~/.local)
#
#   platforms/linux/packaging/make-tarball.sh [path/to/release/jellyfesh] [OUT_DIR]
#
# Without a binary it runs `cargo build --release -p jellyfesh-gtk` first.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
binary=${1:-}
out=${2:-$repo/target/dist}
if [[ -z "$binary" ]]; then
  cargo build --release --locked -p jellyfesh-gtk --manifest-path "$repo/Cargo.toml"
  binary="$repo/target/release/jellyfesh"
fi
version=$(sed -n 's/^version = "\(.*\)"/\1/p' "$repo/Cargo.toml" | head -1)
name="jellyfesh-$version-$(uname -m)"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
"$here/install.sh" --binary "$binary" --prefix / --destdir "$stage/$name"
cat > "$stage/$name/install.sh" <<'EOS'
#!/usr/bin/env bash
# Copies Jellyfesh into a prefix (default ~/.local) and refreshes the icon
# and desktop caches when the tools exist.
set -euo pipefail
prefix=${1:-$HOME/.local}
here="$(cd "$(dirname "$0")" && pwd)"
mkdir -p "$prefix"
cp -r "$here/bin" "$here/share" "$prefix/"
command -v gtk4-update-icon-cache >/dev/null && gtk4-update-icon-cache -q -t -f "$prefix/share/icons/hicolor" || true
command -v update-desktop-database >/dev/null && update-desktop-database -q "$prefix/share/applications" || true
echo "Jellyfesh is in $prefix/bin/jellyfesh"
EOS
chmod +x "$stage/$name/install.sh"
mkdir -p "$out"
tar -C "$stage" -czf "$out/$name.tar.gz" "$name"
echo "$out/$name.tar.gz"
