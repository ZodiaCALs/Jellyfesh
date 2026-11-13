#!/usr/bin/env bash
# Builds the unsigned release APK of Jellyfesh for Android, as CI does:
#
#   1. checks the Android SDK and NDK, adds the Rust Android targets, installs cargo-ndk;
#   2. builds the core (jf-ffi) for arm64-v8a and x86_64 with cargo-ndk into
#      core-bindings/src/main/jniLibs;
#   3. generates the UniFFI Kotlin bindings and the design tokens (cargo xtask);
#   4. notes whether the official plugins (dist/plugins/*.jfpkg) are there to bundle;
#   5. runs ./gradlew :app:assembleRelease and copies the APK to dist/android/.
#
# Usage: platforms/android/scripts/build.sh [--skip-core] [--debug-core]
#   --skip-core   reuse the libraries already in core-bindings/src/main/jniLibs
#   --debug-core  build the core without --release (faster, larger, slower at run time)
#
# Needs: ANDROID_HOME (or ANDROID_SDK_ROOT) with platform 37 and build tools, an NDK (ANDROID_NDK_HOME
# or $ANDROID_HOME/ndk/<version>), JDK 17+, rustup, cmake (libopus), and on Linux the ALSA headers
# for the host build that generates the bindings (libasound2-dev). docs/android.md has the details.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
android="$(cd "$here/.." && pwd)"
root="$(cd "$android/../.." && pwd)"

skip_core=false
profile=release
for arg in "$@"; do
    case "$arg" in
        --skip-core) skip_core=true ;;
        --debug-core) profile=debug ;;
        -h | --help) sed -n '2,20p' "$0"; exit 0 ;;
        *) echo "unknown option: $arg" >&2; exit 2 ;;
    esac
done

note() { printf '\n==> %s\n' "$*"; }
fail() { printf 'error: %s\n' "$*" >&2; exit 1; }

export CARGO_INCREMENTAL=0
cd "$root"

# ---- 1. toolchains ---------------------------------------------------------------------------
note "Android SDK and NDK"
sdk="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}"
[ -n "$sdk" ] && [ -d "$sdk" ] || fail "set ANDROID_HOME to the Android SDK"
export ANDROID_HOME="$sdk"
if [ -z "${ANDROID_NDK_HOME:-}" ]; then
    if [ -d "$sdk/ndk" ]; then
        ANDROID_NDK_HOME="$sdk/ndk/$(ls "$sdk/ndk" | sort -V | tail -1)"
    elif [ -d "$sdk/ndk-bundle" ]; then
        ANDROID_NDK_HOME="$sdk/ndk-bundle"
    fi
fi
[ -n "${ANDROID_NDK_HOME:-}" ] && [ -d "$ANDROID_NDK_HOME" ] || fail "no NDK: install one with sdkmanager or set ANDROID_NDK_HOME"
export ANDROID_NDK_HOME
echo "SDK $ANDROID_HOME"
echo "NDK $ANDROID_NDK_HOME"

if ! $skip_core; then
    command -v cmake >/dev/null || fail "cmake is needed to build libopus for the core"
    note "Rust targets and cargo-ndk"
    rustup target add aarch64-linux-android x86_64-linux-android
    if ! cargo ndk --version >/dev/null 2>&1; then
        cargo install cargo-ndk --version "^4.1" --locked
    fi
    cargo ndk --version

    # ---- 2. the core for both ABIs -------------------------------------------------------------
    note "Building jf-ffi ($profile) for arm64-v8a and x86_64"
    jni="$android/core-bindings/src/main/jniLibs"
    rm -rf "$jni"
    release_flag=()
    [ "$profile" = release ] && release_flag=(--release)
    # API 26 is the app's minimum (cpal's AAudio backend, ADR-0009).
    cargo ndk -t arm64-v8a -t x86_64 --platform 26 -o "$jni" build -p jf-ffi ${release_flag[@]+"${release_flag[@]}"}
    for abi in arm64-v8a x86_64; do
        [ -f "$jni/$abi/libjf_ffi.so" ] || fail "cargo-ndk did not produce $abi/libjf_ffi.so"
    done
fi

# ---- 3. bindings and tokens --------------------------------------------------------------------
note "Generating the Kotlin bindings and tokens"
cargo xtask bindings --lang kotlin
cargo xtask tokens --target kotlin
[ -f "$android/core-bindings/src/main/kotlin/org/jellyfesh/ffi/jellyfesh.kt" ] || fail "the bindings were not generated"

# ---- 4. bundled plugins ------------------------------------------------------------------------
note "Official plugins to bundle"
if compgen -G "$root/dist/plugins/*.jfpkg" >/dev/null; then
    ls -1 "$root"/dist/plugins/*.jfpkg
else
    echo "note: dist/plugins has no .jfpkg files (the plugin packaging script makes them);"
    echo "      the APK is built without bundled plugins."
fi

# ---- 5. the APK --------------------------------------------------------------------------------
note "Assembling the release APK (unsigned)"
cd "$android"
./gradlew --no-daemon --stacktrace -Pjellyfesh.skipCargo=true :app:assembleRelease

apk="$android/app/build/outputs/apk/release/app-release-unsigned.apk"
[ -f "$apk" ] || fail "no APK at $apk"
version="$(sed -n 's/.*versionName = "\(.*\)".*/\1/p' "$android/app/build.gradle.kts" | head -1)"
mkdir -p "$root/dist/android"
out="$root/dist/android/jellyfesh-${version:-dev}-android-unsigned.apk"
cp "$apk" "$out"
note "Done: $out"
