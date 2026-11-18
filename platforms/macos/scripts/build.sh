#!/usr/bin/env bash
# Builds the macOS app from a clean checkout (docs/macos.md):
#
#   1. the Swift bindings and design tokens (cargo xtask bindings / tokens),
#   2. jf-ffi as a static library for arm64 and x86_64, joined into an XCFramework,
#   3. the official plugins (dist/plugins/*.jfpkg, made by plugins/release.sh) copied into the
#      app's resources, which the app hands to the core to install on first start,
#   4. the Xcode project (XcodeGen),
#   5. a universal Release build of Jellyfesh.app, ad-hoc signed, or signed with a Developer ID for
#      notarization (JF_MACOS_SIGN_IDENTITY below),
#   6. a DMG with the app and a link to /Applications (direct downloads; scripts/notarize.sh then
#      notarizes and staples it), or, for the Mac App Store, the app in build/app-store/ for
#      scripts/package-mas.sh.
#
# Needs macOS with Xcode 16 or later, rustup (or both Rust targets installed), CMake (the core
# builds libopus with it) and XcodeGen (installed with Homebrew when missing).
#
# Usage: platforms/macos/scripts/build.sh [--test] [--skip-core]
#   --test        also run the unit tests (Debug, this Mac's architecture)
#   --skip-core   reuse Generated/ from an earlier run (faster when only Swift changed)
# Environment:
#   JF_MACOS_OUT       output folder (default: platforms/macos/build)
#   JF_PLUGINS_DIR     where the official .jfpkg packages are (default: dist/plugins)
#   CARGO_TARGET_DIR   honoured as usual
#   JF_EDITION         community (default) or commercial; commercial builds bundle no unofficial
#                      community plugins (ADR-0021 §4)
#   JF_DISTRIBUTION    source (default), direct or app-store: the app's Info.plist keys JFEdition and
#                      JFDistribution and Swift conditions JF_EDITION_* and JF_DISTRIBUTION_*
#                      (project.yml); app-store builds are sandboxed (Jellyfesh-AppStore.entitlements)
#                      and make no DMG
#   JF_MACOS_SIGN_IDENTITY  a "Developer ID Application" identity: the direct download's app and DMG
#                      are signed with it (hardened runtime, secure timestamp) instead of ad hoc;
#                      JF_MACOS_KEYCHAIN names its keychain when it is not in the search list
#                      (scripts/keychain.sh makes one in CI)
#   JF_WEBSITE         the website About links to, with its third-party licences page; in GitHub
#                      Actions it defaults to https://OWNER.github.io/NAME/ (GITHUB_REPOSITORY)
#
# Written for the bash 3.2 that macOS ships.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
macos="$(cd "$here/.." && pwd)"
root="$(cd "$macos/../.." && pwd)"
out="${JF_MACOS_OUT:-$macos/build}"
generated="$macos/Generated"
targets="aarch64-apple-darwin x86_64-apple-darwin"
run_tests=0
skip_core=0
for arg in "$@"; do
  case "$arg" in
    --test) run_tests=1 ;;
    --skip-core) skip_core=1 ;;
    -h | --help) sed -n '2,36p' "$0"; exit 0 ;;
    *) echo "unknown argument: $arg" >&2; exit 2 ;;
  esac
done

edition="${JF_EDITION:-community}"
distribution="${JF_DISTRIBUTION:-source}"
case "$edition" in
  community | commercial) ;;
  *) echo "JF_EDITION must be community or commercial, not '$edition'" >&2; exit 2 ;;
esac
case "$distribution" in
  source | direct | app-store) ;;
  *) echo "JF_DISTRIBUTION must be source, direct or app-store, not '$distribution'" >&2; exit 2 ;;
esac
sign_identity="${JF_MACOS_SIGN_IDENTITY:--}"
if [ "$distribution" = app-store ] && [ "$sign_identity" != "-" ]; then
  echo "JF_MACOS_SIGN_IDENTITY signs direct downloads; scripts/package-mas.sh signs App Store builds" >&2
  exit 2
fi
keychain_args=()
if [ -n "${JF_MACOS_KEYCHAIN:-}" ]; then keychain_args=(--keychain "$JF_MACOS_KEYCHAIN"); fi
# The Swift conditions of the edition and distribution, e.g. JF_EDITION_COMMERCIAL JF_DISTRIBUTION_APP_STORE.
edition_conditions="JF_EDITION_$(printf '%s' "$edition" | tr 'a-z-' 'A-Z_') JF_DISTRIBUTION_$(printf '%s' "$distribution" | tr 'a-z-' 'A-Z_')"
# The website About links to (project.yml JF_WEBSITE): as given, else the repository's GitHub Pages
# address in GitHub Actions, where web.yml publishes the website (docs/github.md), else none.
website="${JF_WEBSITE:-}"
if [ -z "$website" ] && [ -n "${GITHUB_REPOSITORY:-}" ]; then
  repo_owner="${GITHUB_REPOSITORY%%/*}"
  repo_name="${GITHUB_REPOSITORY#*/}"
  if [ -n "$repo_owner" ] && [ -n "$repo_name" ] && [ "$repo_name" = "${repo_name#*/}" ]; then
    website="https://$(printf '%s' "$repo_owner" | tr 'A-Z' 'a-z').github.io/$repo_name/"
  fi
fi
entitlements="Jellyfesh/Jellyfesh.entitlements"
if [ "$distribution" = app-store ]; then entitlements="Jellyfesh/Jellyfesh-AppStore.entitlements"; fi

# The deployment target of the app (project.yml) applies to the Rust and C objects too.
export MACOSX_DEPLOYMENT_TARGET=14.2
# whisper.cpp, which jf-ffi builds through whisper-rs-sys and CMake for on-device AI Lyrics
# (ADR-0019): never tuned to the CPU of the Mac that builds it, so each slice runs on every Mac it is
# for. The x86_64 slice asks for the AVX2 family instead, which every Intel Mac that runs macOS 14.2
# has; without it that slice would be unoptimised.
export GGML_NATIVE=OFF
# The extra settings for a target's build of the core, in the array ggml_env (empty for arm64).
ggml_env_for() {
  case "$1" in
    x86_64-*) ggml_env=(GGML_AVX=ON GGML_AVX2=ON GGML_FMA=ON GGML_F16C=ON) ;;
    *) ggml_env=() ;;
  esac
}
version="$(sed -n 's/^    MARKETING_VERSION: //p' "$macos/project.yml")"
mkdir -p "$out"

step() { printf '\n==> %s\n' "$*"; }

cd "$root"
target_dir="${CARGO_TARGET_DIR:-$root/target}"
link_flags=""

if [ "$skip_core" -eq 0 ]; then
  step "Rust targets"
  if command -v rustup >/dev/null 2>&1; then
    # shellcheck disable=SC2086
    rustup target add $targets
  fi
  host="$(rustc -vV | sed -n 's/^host: //p')"

  step "Bindings and tokens"
  # Builds jf-ffi in release for this Mac (target/release) and writes Generated/.
  ggml_env_for "$host"
  env ${ggml_env[@]+"${ggml_env[@]}"} cargo xtask bindings --lang swift --release
  cargo xtask tokens --target swift

  step "Static libraries"
  libs=""
  for target in $targets; do
    if [ "$target" = "$host" ]; then
      lib="$target_dir/release/libjf_ffi.a"
    else
      notes="$out/rustc-$target.log"
      # Without colour (CI sets CARGO_TERM_COLOR=always): the flags are read from this output,
      # and a colour code after the last one would reach the linker as part of a library name.
      ggml_env_for "$target"
      env ${ggml_env[@]+"${ggml_env[@]}"} cargo rustc --color never --release --locked --package jf-ffi --lib \
        --crate-type staticlib --target "$target" -- --print native-static-libs 2>&1 | tee "$notes"
      flags="$(tr -d '\033' < "$notes" | sed -n 's/.*native-static-libs: //p' | sed 's/\[[0-9;]*m//g' | tail -n 1)"
      if [ -n "$flags" ]; then link_flags="$flags"; fi
      lib="$target_dir/$target/release/libjf_ffi.a"
    fi
    if [ ! -f "$lib" ]; then
      echo "missing $lib" >&2
      exit 1
    fi
    libs="$libs $lib"
  done

  step "XCFramework"
  mkdir -p "$out/universal"
  # shellcheck disable=SC2086
  lipo -create $libs -output "$out/universal/libjf_ffi.a"
  lipo -info "$out/universal/libjf_ffi.a"
  rm -rf "$generated/JellyfeshCore.xcframework"
  xcodebuild -create-xcframework -library "$out/universal/libjf_ffi.a" \
    -output "$generated/JellyfeshCore.xcframework"
  # The C half of the bindings, as a Clang module the Swift targets import (project.yml).
  rm -rf "$generated/include"
  mkdir -p "$generated/include"
  cp "$generated/JellyfeshFFIFFI.h" "$generated/include/JellyfeshFFIFFI.h"
  cp "$generated/JellyfeshFFIFFI.modulemap" "$generated/include/module.modulemap"
  if [ -n "$link_flags" ]; then
    printf '%s\n' "$link_flags" > "$generated/native-static-libs.txt"
  fi
elif [ -f "$generated/native-static-libs.txt" ]; then
  link_flags="$(cat "$generated/native-static-libs.txt")"
fi

for file in JellyfeshFFI.swift Tokens.swift include/module.modulemap JellyfeshCore.xcframework; do
  if [ ! -e "$generated/$file" ]; then
    echo "Generated/$file is missing: run without --skip-core first" >&2
    exit 1
  fi
done

# Features of the core the generated bindings already have, as Swift compilation conditions
# (Sources/CoreClient checks them; project.yml adds $(JF_SWIFT_CONDITIONS) to the app's).
conditions=""
if grep -q 'bundledPlugins' "$generated/JellyfeshFFI.swift"; then
  conditions="JF_BUNDLED_PLUGINS"
fi

step "Official plugins"
# Bundled in Resources/Plugins (project.yml: Generated/Plugins) and installed by the core on first
# start; they also ship as separate .jfpkg downloads. The commercial edition leaves out the
# unofficial community plugins (community.unofficial.*), which stay separate downloads.
plugins_dir="${JF_PLUGINS_DIR:-$root/dist/plugins}"
rm -rf "$generated/Plugins"
# The folder is always there, empty without packages: the project copies it into the app, and
# Xcode fails on a folder that does not exist.
mkdir -p "$generated/Plugins"
packages=""
if [ -d "$plugins_dir" ]; then
  packages="$(find "$plugins_dir" -maxdepth 1 -name '*.jfpkg' -type f | sort)"
fi
if [ -n "$packages" ]; then
  printf '%s\n' "$packages" | while IFS= read -r package; do
    name="$(basename "$package")"
    if [ "$edition" = commercial ]; then
      case "$name" in
        community.unofficial.*) echo "commercial edition: not bundling $name"; continue ;;
      esac
    fi
    cp "$package" "$generated/Plugins/"
    echo "bundling $name"
  done
  if [ -z "$conditions" ]; then
    echo "note: these bindings have no CoreConfig.bundledPlugins yet; the app carries the" \
      "packages but cannot hand them to the core" >&2
  fi
else
  echo "note: no official plugins in $plugins_dir (run plugins/release.sh first);" \
    "building without bundled plugins" >&2
fi

step "Xcode project"
if ! command -v xcodegen >/dev/null 2>&1; then
  brew install xcodegen
fi
(cd "$macos" && xcodegen generate --spec project.yml)

# The system libraries rustc reported for the core's static library (project.yml has a default).
extra=""
if [ -n "$link_flags" ]; then
  extra="JF_RUST_LDFLAGS=$link_flags"
  echo "linking the core with: $link_flags"
fi
swift_conditions="JF_SWIFT_CONDITIONS=$conditions"
if [ -n "$conditions" ]; then
  echo "Swift conditions: $conditions"
fi
echo "edition $edition, distribution $distribution ($edition_conditions)"
echo "website: ${website:-none}"
edition_settings=("JF_EDITION=$edition" "JF_DISTRIBUTION=$distribution"
  "JF_EDITION_CONDITIONS=$edition_conditions" "JF_ENTITLEMENTS=$entitlements")

derived="$out/DerivedData"
if [ "$run_tests" -eq 1 ]; then
  step "Unit tests"
  xcodebuild -project "$macos/Jellyfesh.xcodeproj" -scheme Jellyfesh -configuration Debug \
    -destination "platform=macOS,arch=$(uname -m)" -derivedDataPath "$derived" \
    ${extra:+"$extra"} "$swift_conditions" "${edition_settings[@]}" "JF_WEBSITE=$website" test
fi

step "Release build"
xcodebuild -project "$macos/Jellyfesh.xcodeproj" -scheme Jellyfesh -configuration Release \
  -destination "generic/platform=macOS" -derivedDataPath "$derived" \
  ARCHS="arm64 x86_64" ONLY_ACTIVE_ARCH=NO \
  CODE_SIGN_IDENTITY=- CODE_SIGN_STYLE=Manual DEVELOPMENT_TEAM= \
  ${extra:+"$extra"} "$swift_conditions" "${edition_settings[@]}" "JF_WEBSITE=$website" build

app="$derived/Build/Products/Release/Jellyfesh.app"
echo "architectures: $(lipo -archs "$app/Contents/MacOS/Jellyfesh")"
if [ "$sign_identity" != "-" ]; then
  step "Developer ID signature"
  # What notarization asks for: the hardened runtime and a secure timestamp. The app has one
  # executable and no nested code (the core and the bindings are linked in statically).
  codesign --force --options runtime --timestamp --entitlements "$macos/$entitlements" \
    --sign "$sign_identity" ${keychain_args[@]+"${keychain_args[@]}"} "$app"
fi
codesign --verify --strict --verbose=2 "$app"

if [ "$distribution" = app-store ]; then
  rm -rf "$out/app-store"
  mkdir -p "$out/app-store"
  ditto "$app" "$out/app-store/Jellyfesh.app"
  echo "built $out/app-store/Jellyfesh.app for the Mac App Store (scripts/package-mas.sh signs and packs it)"
  exit 0
fi

step "DMG"
stage="$out/dmg"
rm -rf "$stage"
mkdir -p "$stage"
ditto "$app" "$stage/Jellyfesh.app"
ln -s /Applications "$stage/Applications"
dmg="$out/Jellyfesh-$version.dmg"
rm -f "$dmg"
hdiutil create -volname "Jellyfesh $version" -srcfolder "$stage" -ov -format UDZO "$dmg"
if [ "$sign_identity" != "-" ]; then
  codesign --force --timestamp --sign "$sign_identity" ${keychain_args[@]+"${keychain_args[@]}"} "$dmg"
  codesign --verify --verbose=2 "$dmg"
fi
(cd "$out" && shasum -a 256 "$(basename "$dmg")" > "$(basename "$dmg").sha256")
rm -rf "$stage"
echo "built $dmg"
