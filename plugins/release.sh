#!/usr/bin/env bash
# Builds the official plugins in release mode and writes their packages:
#
#   dist/plugins/org.jellyfesh.subsonic.jfpkg
#   dist/plugins/org.jellyfesh.jellyfin.jfpkg
#   dist/plugins/org.jellyfesh.qqmusic.jfpkg
#   dist/plugins/org.jellyfesh.spotify.jfpkg
#   dist/plugins/org.jellyfesh.netease.jfpkg
#
# Signing: when JF_PLUGIN_SIGNING_KEY is set and not empty, it must hold the
# Jellyfesh release key (64 hex characters, the ed25519 seed; a trailing
# newline is fine). The packages are signed with it and must then verify as
# Official, or the script fails: a wrong key would ship packages the apps do
# not trust. Set JF_PLUGIN_ALLOW_UNTRUSTED_KEY=1 to sign with another key
# anyway (a fork, a dry run). When JF_PLUGIN_SIGNING_KEY is unset or empty,
# the packages are unsigned.
#
# Run it from the repository root. It needs only Rust with the
# wasm32-unknown-unknown target; it builds the jf-plugin tool on the way.
# See docs/plugins/authoring.md.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

out=dist/plugins
plugins=(subsonic jellyfin qqmusic spotify netease)

if [ -n "${JF_PLUGIN_SIGNING_KEY:-}" ]; then
    sign=yes
    echo "release.sh: signing the packages with the key in JF_PLUGIN_SIGNING_KEY"
else
    sign=no
    echo "release.sh: JF_PLUGIN_SIGNING_KEY is not set; the packages will be unsigned"
fi

# The tool first, with cargo's progress shown; then run it quietly.
cargo build --locked --package jf-plugin
jf_plugin() {
    cargo run --locked --quiet --package jf-plugin -- "$@"
}

mkdir -p "$out"
for name in "${plugins[@]}"; do
    package="$out/org.jellyfesh.$name.jfpkg"
    rm -f "$package"
    jf_plugin pack --release --path "plugins/$name" --out "$package"
    if [ "$sign" = yes ]; then
        jf_plugin sign "$package"
        if [ "${JF_PLUGIN_ALLOW_UNTRUSTED_KEY:-}" = 1 ]; then
            jf_plugin verify --require-signature "$package"
        elif ! jf_plugin verify --require-trusted "$package"; then
            rm -f "$package"
            echo "release.sh: the key in JF_PLUGIN_SIGNING_KEY is not the Jellyfesh release key" >&2
            exit 1
        fi
        echo "release.sh: $package is signed"
    else
        jf_plugin verify "$package"
        echo "release.sh: $package is unsigned"
    fi
done
ls -l "$out"/org.jellyfesh.*.jfpkg
