#!/usr/bin/env bash
# Builds the unsigned release APK of Jellyfesh for Android, as CI does:
#
#   1. checks the Android SDK and NDK, adds the Rust Android targets, installs cargo-ndk;
#   2. builds the core (jf-ffi, with whisper.cpp for on-device AI Lyrics) for arm64-v8a and x86_64
#      with cargo-ndk into core-bindings/src/main/jniLibs, with the NDK's libc++_shared.so beside it;
#   3. generates the UniFFI Kotlin bindings and the design tokens (cargo xtask);
#   4. notes whether the official plugins (dist/plugins/*.jfpkg) are there to bundle;
#   5. runs ./gradlew :app:assembleRelease, checks that the APK holds libjf_ffi.so and
#      libc++_shared.so for both ABIs, and copies it to dist/android/;
#   6. with --play-bundle, also ./gradlew :app:bundleRelease for Google Play (the distribution
#      google-play), checked the same way, copied to dist/android/ as
#      jellyfesh-<version>-google-play-unsigned.aab.
#
# Usage: platforms/android/scripts/build.sh [--skip-core] [--debug-core] [--play-bundle]
#   --skip-core    reuse the libraries already in core-bindings/src/main/jniLibs
#   --debug-core   build the core without --release (faster, larger, slower at run time)
#   --play-bundle  also build the unsigned App Bundle for Google Play
#
# Edition and distribution (ADR-0021, docs/github.md; app/build.gradle.kts): JF_EDITION (community,
# the default, or commercial: no unofficial community plugins bundled), JF_DISTRIBUTION of the APK
# (source, the default, or direct) and PLAY_LICENSE_KEY, from the environment.
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
play_bundle=false
for arg in "$@"; do
    case "$arg" in
        --skip-core) skip_core=true ;;
        --debug-core) profile=debug ;;
        --play-bundle) play_bundle=true ;;
        -h | --help) sed -n '2,26p' "$0"; exit 0 ;;
        *) echo "unknown option: $arg" >&2; exit 2 ;;
    esac
done

note() { printf '\n==> %s\n' "$*"; }
fail() { printf 'error: %s\n' "$*" >&2; exit 1; }

# The core's native libraries for each ABI inside a package: the APK keeps them under lib/<abi>/,
# the App Bundle under base/lib/<abi>/. libjf_ffi.so needs libc++_shared.so beside it (whisper.cpp
# links the C++ runtime dynamically), or it fails to load in the app.
check_native_libs() {
    local package="$1" prefix="$2" names
    names="$(unzip -Z1 "$package")" || fail "cannot list $package"
    for abi in arm64-v8a x86_64; do
        for lib in libjf_ffi.so libc++_shared.so; do
            grep -qxF "${prefix}lib/$abi/$lib" <<< "$names" || fail "$package has no ${prefix}lib/$abi/$lib"
        done
    done
    echo "native libraries present for arm64-v8a and x86_64: libjf_ffi.so, libc++_shared.so"
}

export CARGO_INCREMENTAL=0
# whisper.cpp (ggml, built by whisper-rs-sys with CMake, which reads GGML_* variables) targets each
# ABI's baseline CPU, never the build machine's own instruction set.
export GGML_NATIVE=OFF
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
    # API 26 is the app's minimum (cpal's AAudio backend, ADR-0009). whisper-rs-sys links the C++
    # runtime as c++_shared: --link-libcxx-shared links it and copies the NDK's libc++_shared.so
    # next to libjf_ffi.so in each ABI's folder, so the APK ships it.
    cargo ndk -t arm64-v8a -t x86_64 --platform 26 --link-libcxx-shared -o "$jni" build -p jf-ffi ${release_flag[@]+"${release_flag[@]}"}
    for abi in arm64-v8a x86_64; do
        [ -f "$jni/$abi/libjf_ffi.so" ] || fail "cargo-ndk did not produce $abi/libjf_ffi.so"
        [ -f "$jni/$abi/libc++_shared.so" ] || fail "cargo-ndk did not copy $abi/libc++_shared.so"
    done
fi

# ---- 3. bindings and tokens --------------------------------------------------------------------
note "Generating the Kotlin bindings and tokens"
cargo xtask bindings --lang kotlin
cargo xtask tokens --target kotlin
[ -f "$android/core-bindings/src/main/kotlin/org/jellyfesh/ffi/jellyfesh.kt" ] || fail "the bindings were not generated"

# ---- 4. bundled plugins ------------------------------------------------------------------------
note "Official plugins to bundle (edition ${JF_EDITION:-community})"
if compgen -G "$root/dist/plugins/*.jfpkg" >/dev/null; then
    for package in "$root"/dist/plugins/*.jfpkg; do
        case "${JF_EDITION:-community}:$(basename "$package")" in
            commercial:community.unofficial.*) echo "not bundled (commercial edition): $package" ;;
            *) echo "$package" ;;
        esac
    done
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
check_native_libs "$apk" ""
version="$(sed -n 's/.*versionName = "\(.*\)".*/\1/p' "$android/app/build.gradle.kts" | head -1)"
mkdir -p "$root/dist/android"
out="$root/dist/android/jellyfesh-${version:-dev}-android-unsigned.apk"
cp "$apk" "$out"
note "Done: $out"

# ---- 6. the App Bundle for Google Play ---------------------------------------------------------
if $play_bundle; then
    note "Assembling the App Bundle for Google Play (unsigned)"
    ./gradlew --no-daemon --stacktrace -Pjellyfesh.skipCargo=true -Pjellyfesh.distribution=google-play \
        :app:bundleRelease
    aab="$android/app/build/outputs/bundle/release/app-release.aab"
    [ -f "$aab" ] || fail "no App Bundle at $aab"
    check_native_libs "$aab" "base/"
    out="$root/dist/android/jellyfesh-${version:-dev}-google-play-unsigned.aab"
    cp "$aab" "$out"
    note "Done: $out"
fi
