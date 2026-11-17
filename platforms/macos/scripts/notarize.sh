#!/usr/bin/env bash
# Notarizes a signed DMG with Apple's notary service and staples the ticket to it (ADR-0021 §7,
# docs/store-release.md), for the direct download:
#
#   platforms/macos/scripts/notarize.sh platforms/macos/build/Jellyfesh-<version>.dmg
#
# The DMG and the app in it must be signed with a Developer ID (scripts/build.sh with
# JF_MACOS_SIGN_IDENTITY). The notary service checks the app inside as well, and Gatekeeper finds
# the ticket stapled to the DMG even offline.
#
# Signs in to the notary service with an App Store Connect API key: APPLE_NOTARY_KEY_ID,
# APPLE_NOTARY_ISSUER_ID and APPLE_NOTARY_KEY (the text of the AuthKey_<id>.p8 file). Without them
# it says so (a GitHub Actions notice) and exits 0, leaving the DMG unnotarized. It waits for the
# result, prints the notary log, and fails unless the submission is Accepted.
#
# Written for the bash 3.2 that macOS ships.
set -euo pipefail

dmg="${1:-}"
if [ -z "$dmg" ] || [ "$dmg" = -h ] || [ "$dmg" = --help ]; then
  sed -n '2,16p' "$0"
  exit 2
fi

missing=""
for name in APPLE_NOTARY_KEY_ID APPLE_NOTARY_ISSUER_ID APPLE_NOTARY_KEY; do
  if [ -z "${!name:-}" ]; then missing="$missing $name"; fi
done
if [ -n "$missing" ]; then
  echo "::notice title=Not notarized::$(basename "$dmg") is not notarized: the secrets$missing are not set (docs/store-release.md)"
  exit 0
fi
if [ ! -f "$dmg" ]; then
  echo "notarize.sh: $dmg does not exist" >&2
  exit 2
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
key="$work/AuthKey_$APPLE_NOTARY_KEY_ID.p8"
(umask 077 && printf '%s\n' "$APPLE_NOTARY_KEY" > "$key")
auth=(--key "$key" --key-id "$APPLE_NOTARY_KEY_ID" --issuer "$APPLE_NOTARY_ISSUER_ID")

echo "notarize.sh: submitting $(basename "$dmg") and waiting for the result"
status=0
xcrun notarytool submit "$dmg" "${auth[@]}" --wait --output-format json > "$work/result.json" || status=$?
cat "$work/result.json"
echo
result="$(plutil -extract status raw -o - "$work/result.json" 2> /dev/null || true)"
id="$(plutil -extract id raw -o - "$work/result.json" 2> /dev/null || true)"
if [ -n "$id" ]; then
  # Warnings come with an accepted submission too: worth reading before the next one.
  xcrun notarytool log "$id" "${auth[@]}" || true
fi
if [ "$result" != Accepted ]; then
  echo "notarize.sh: the notary service answered '${result:-nothing}' (notarytool exit status $status)" >&2
  exit 1
fi

xcrun stapler staple "$dmg"
xcrun stapler validate "$dmg"
# Gatekeeper's verdict on the disk image as a browser download.
spctl --assess --type open --context context:primary-signature --verbose=2 "$dmg"
# Stapling changed the file: its checksum from scripts/build.sh, if any, is written again.
if [ -f "$dmg.sha256" ]; then
  (cd "$(dirname "$dmg")" && shasum -a 256 "$(basename "$dmg")" > "$(basename "$dmg").sha256")
fi
echo "notarize.sh: $(basename "$dmg") is notarized and stapled"
