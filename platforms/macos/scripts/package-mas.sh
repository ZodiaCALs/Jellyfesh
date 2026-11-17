#!/usr/bin/env bash
# Signs the Mac App Store build of Jellyfesh and packs it as an installer package for App Store
# Connect (ADR-0021 §7, docs/store-release.md):
#
#   JF_EDITION=commercial JF_DISTRIBUTION=app-store platforms/macos/scripts/build.sh
#   platforms/macos/scripts/package-mas.sh platforms/macos/build/app-store/Jellyfesh.app OUT.pkg
#
# Needs, in a keychain in the search list (scripts/keychain.sh makes one in CI; JF_MACOS_KEYCHAIN
# names it):
#   JF_MAS_APP_IDENTITY        "Apple Distribution: Name (TEAM)" (or the older "3rd Party Mac
#                              Developer Application: Name (TEAM)"), which signs the app;
#   JF_MAS_INSTALLER_IDENTITY  "3rd Party Mac Developer Installer: Name (TEAM)", from a Mac Installer
#                              Distribution certificate, which signs the package;
# and the Mac App Store provisioning profile of org.jellyfesh.Jellyfesh: JF_MAS_PROFILE (the file)
# or APPLE_MAS_PROFILE (its base64). Without them it says so (a GitHub Actions notice) and exits 0.
#
# The app is signed with Jellyfesh/Jellyfesh-AppStore.entitlements (the sandbox) plus the
# application and team identifiers from the profile, which goes into the app as
# Contents/embedded.provisionprofile; productbuild then makes the signed package. Upload it with
# Transporter or `xcrun altool --upload-package` (docs/store-release.md).
#
# Written for the bash 3.2 that macOS ships.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
macos="$(cd "$here/.." && pwd)"
app="${1:-}"
pkg="${2:-}"
if [ -z "$app" ] || [ -z "$pkg" ]; then
  sed -n '2,24p' "$0"
  exit 2
fi

missing=""
if [ -z "${JF_MAS_APP_IDENTITY:-}" ]; then missing="$missing JF_MAS_APP_IDENTITY"; fi
if [ -z "${JF_MAS_INSTALLER_IDENTITY:-}" ]; then missing="$missing JF_MAS_INSTALLER_IDENTITY"; fi
if [ -z "${JF_MAS_PROFILE:-}" ] && [ -z "${APPLE_MAS_PROFILE:-}" ]; then missing="$missing APPLE_MAS_PROFILE"; fi
if [ -n "$missing" ]; then
  echo "::notice title=No Mac App Store package::Not set:$missing (the Mac App Store certificates and profile, docs/store-release.md)"
  exit 0
fi
if [ ! -d "$app" ]; then
  echo "package-mas.sh: no app at $app (build it with JF_DISTRIBUTION=app-store scripts/build.sh)" >&2
  exit 2
fi
keychain_args=()
if [ -n "${JF_MACOS_KEYCHAIN:-}" ]; then keychain_args=(--keychain "$JF_MACOS_KEYCHAIN"); fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
profile="${JF_MAS_PROFILE:-$work/profile.provisionprofile}"
if [ -z "${JF_MAS_PROFILE:-}" ]; then
  printf '%s' "$APPLE_MAS_PROFILE" | base64 --decode > "$profile"
fi

# The identifiers the profile allows, which the signature must carry.
security cms -D -i "$profile" > "$work/profile.plist"
plist() { /usr/libexec/PlistBuddy -c "Print :$1" "$2"; }
app_id="$(plist Entitlements:com.apple.application-identifier "$work/profile.plist")"
team_id="$(plist Entitlements:com.apple.developer.team-identifier "$work/profile.plist")"
bundle_id="$(plist CFBundleIdentifier "$app/Contents/Info.plist")"
if [ "$app_id" != "$team_id.$bundle_id" ]; then
  echo "package-mas.sh: the profile is for $app_id, not $team_id.$bundle_id" >&2
  exit 1
fi
distribution="$(plist JFDistribution "$app/Contents/Info.plist" 2> /dev/null || true)"
if [ "$distribution" != app-store ]; then
  echo "package-mas.sh: $app is a '$distribution' build; build it with JF_DISTRIBUTION=app-store" >&2
  exit 1
fi

entitlements="$work/entitlements.plist"
cp "$macos/Jellyfesh/Jellyfesh-AppStore.entitlements" "$entitlements"
/usr/libexec/PlistBuddy -c "Add :com.apple.application-identifier string $app_id" "$entitlements"
/usr/libexec/PlistBuddy -c "Add :com.apple.developer.team-identifier string $team_id" "$entitlements"

cp "$profile" "$app/Contents/embedded.provisionprofile"
codesign --force --options runtime --timestamp --entitlements "$entitlements" \
  --sign "$JF_MAS_APP_IDENTITY" ${keychain_args[@]+"${keychain_args[@]}"} "$app"
codesign --verify --strict --deep --verbose=2 "$app"
codesign --display --entitlements - "$app"

mkdir -p "$(dirname "$pkg")"
rm -f "$pkg"
productbuild --component "$app" /Applications --sign "$JF_MAS_INSTALLER_IDENTITY" \
  ${keychain_args[@]+"${keychain_args[@]}"} "$pkg"
pkgutil --check-signature "$pkg"
echo "package-mas.sh: wrote $pkg for App Store Connect"
