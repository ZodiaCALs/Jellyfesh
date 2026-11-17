#!/usr/bin/env bash
# Builds the plugins the apps bundle in release mode and writes their packages:
#
#   dist/plugins/org.jellyfesh.subsonic.jfpkg         the official plugins
#   dist/plugins/org.jellyfesh.jellyfin.jfpkg
#   dist/plugins/org.jellyfesh.qqmusic.jfpkg
#   dist/plugins/org.jellyfesh.spotify.jfpkg
#   dist/plugins/org.jellyfesh.netease.jfpkg
#   dist/plugins/community.unofficial.netease.jfpkg   the community plugins of
#   dist/plugins/community.unofficial.qqmusic.jfpkg   plugins/community, unsigned
#
# dist/plugins is what every app bundles. Edition (ADR-0021 §4): with JF_EDITION=commercial (set
# by the release workflow) the community packages go to dist/community-plugins/ instead, as
# separate downloads that commercial builds do not bundle, and any left in dist/plugins by an
# earlier build are removed. JF_EDITION=community, the default for builds from source, bundles
# them as in v0.0.6.
#
# Signing: when JF_PLUGIN_SIGNING_KEY is set and not empty, it must hold the
# Jellyfesh release key (64 hex characters, the ed25519 seed; a trailing
# newline is fine). The packages are signed with it and must then verify as
# Official, or the script fails: a wrong key would ship packages the apps do
# not trust. Set JF_PLUGIN_ALLOW_UNTRUSTED_KEY=1 to sign with another key
# anyway (a fork, a dry run). When JF_PLUGIN_SIGNING_KEY is unset or empty,
# the packages are unsigned.
#
# The community plugins (ADR-0012, amended for v0.0.6) are never signed, with
# the release key or any other: they are built and packed without
# JF_PLUGIN_SIGNING_KEY in their environment, and the script fails if one
# comes out signed or with the tests' `mock_origin` setting (their
# `mock-service` feature, which this script never turns on). The apps install
# them only once the person allows community plugins.
#
# Run it from the repository root. It needs only Rust with the
# wasm32-unknown-unknown target; it builds the jf-plugin tool on the way.
# See docs/plugins/authoring.md.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

out=dist/plugins
plugins=(subsonic jellyfin qqmusic spotify netease)
edition="${JF_EDITION:-community}"
case "$edition" in
    community) community_out="$out" ;;
    commercial) community_out=dist/community-plugins ;;
    *)
        echo "release.sh: JF_EDITION must be community or commercial, not '$edition'" >&2
        exit 2
        ;;
esac
# plugins/community/<name>, packed as community.unofficial.<name>.jfpkg
community=(netease qqmusic)

if [ -n "${JF_PLUGIN_SIGNING_KEY:-}" ]; then
    sign=yes
    echo "release.sh: signing the official packages with the key in JF_PLUGIN_SIGNING_KEY (never the community ones)"
else
    sign=no
    echo "release.sh: JF_PLUGIN_SIGNING_KEY is not set; the packages will be unsigned"
fi

# The tool first, with cargo's progress shown; then run it quietly.
cargo build --locked --package jf-plugin
jf_plugin() {
    cargo run --locked --quiet --package jf-plugin -- "$@"
}
# The same for the community plugins, whose builds never see the release key.
jf_plugin_community() {
    env -u JF_PLUGIN_SIGNING_KEY cargo run --locked --quiet --package jf-plugin -- "$@"
}

mkdir -p "$out" "$community_out"
if [ "$edition" = commercial ]; then
    echo "release.sh: commercial edition: the community packages go to $community_out, not $out"
    rm -f "$out"/community.unofficial.*.jfpkg
fi
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

for name in "${community[@]}"; do
    package="$community_out/community.unofficial.$name.jfpkg"
    rm -f "$package"
    jf_plugin_community pack --release --path "plugins/community/$name" --out "$package"
    report=$(jf_plugin_community verify "$package")
    echo "$report"
    if ! grep -q '^signature: none: unsigned' <<< "$report"; then
        rm -f "$package"
        echo "release.sh: $package is signed; a community plugin must ship unsigned" >&2
        exit 1
    fi
    inspected=$(jf_plugin_community inspect --json "$package")
    if grep -q mock_origin <<< "$inspected"; then
        rm -f "$package"
        echo "release.sh: $package has the tests' mock_origin setting" >&2
        exit 1
    fi
    echo "release.sh: $package is unsigned (community plugin)"
done
ls -l "$out"/org.jellyfesh.*.jfpkg "$community_out"/community.unofficial.*.jfpkg
