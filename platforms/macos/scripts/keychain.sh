#!/usr/bin/env bash
# Puts the signing certificates of a release build into a temporary keychain (macOS, CI; ADR-0021
# §7, docs/store-release.md):
#
#   platforms/macos/scripts/keychain.sh ENV_FILE    # in GitHub Actions: "$GITHUB_ENV"
#   platforms/macos/scripts/keychain.sh --delete    # removes the keychain again
#
# Reads the certificates as base64 .p12 files from the environment, imports those that are set, and
# appends what the other scripts read to ENV_FILE, one NAME=value per line:
#
#   APPLE_DEVELOPER_ID_P12, APPLE_DEVELOPER_ID_PASSWORD
#       -> JF_MACOS_SIGN_IDENTITY     "Developer ID Application: Name (TEAM)" (scripts/build.sh)
#   APPLE_MAS_APP_P12, APPLE_MAS_INSTALLER_P12, APPLE_MAS_PASSWORD
#       -> JF_MAS_APP_IDENTITY        "Apple Distribution: Name (TEAM)" or "3rd Party Mac Developer
#                                     Application: Name (TEAM)" (scripts/package-mas.sh)
#       -> JF_MAS_INSTALLER_IDENTITY  "3rd Party Mac Developer Installer: Name (TEAM)"
#   and JF_MACOS_KEYCHAIN, the keychain, once anything was imported.
#
# With APPLE_TEAM_ID set, only identities of that team are used. A certificate whose secrets are not
# set is skipped with a GitHub Actions notice; nothing fails for a missing secret.
#
# Written for the bash 3.2 that macOS ships.
set -euo pipefail

keychain="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/jellyfesh-signing.keychain-db"
case "${1:-}" in
  --delete)
    if [ -f "$keychain" ]; then security delete-keychain "$keychain"; fi
    exit 0
    ;;
  "" | -h | --help)
    sed -n '2,22p' "$0"
    exit 2
    ;;
esac
env_file="$1"

notice() { echo "::notice title=$1::$2"; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
password=""

# Creates the keychain on first use: unlocked for six hours, in the user's search list so that
# codesign and productbuild find the identities and their certificate chains.
make_keychain() {
  if [ -n "$password" ]; then return; fi
  password="$(uuidgen)"
  if [ "${GITHUB_ACTIONS:-}" = true ]; then echo "::add-mask::$password"; fi
  if [ -f "$keychain" ]; then security delete-keychain "$keychain"; fi
  security create-keychain -p "$password" "$keychain"
  security set-keychain-settings -lut 21600 "$keychain"
  security unlock-keychain -p "$password" "$keychain"
  local list=("$keychain") line
  while IFS= read -r line; do
    line="${line#"${line%%[![:space:]]*}"}"
    line="${line#\"}"
    line="${line%\"}"
    if [ -n "$line" ]; then list+=("$line"); fi
  done < <(security list-keychains -d user)
  security list-keychains -d user -s "${list[@]}"
}

# import BASE64 PASSWORD: one .p12 (certificate and private key) into the keychain.
import() {
  make_keychain
  printf '%s' "$1" | base64 --decode > "$work/certificate.p12"
  security import "$work/certificate.p12" -k "$keychain" -f pkcs12 -P "$2" \
    -T /usr/bin/codesign -T /usr/bin/productbuild -T /usr/bin/productsign -T /usr/bin/security > /dev/null
  rm -f "$work/certificate.p12"
  # Lets the tools use the keys without a dialog.
  security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$password" "$keychain" > /dev/null
}

# identity PATTERN: the name of the first identity in the keychain that matches PATTERN (and the
# team in APPLE_TEAM_ID), or nothing.
identity() {
  local team=".*"
  if [ -n "${APPLE_TEAM_ID:-}" ]; then team="$APPLE_TEAM_ID"; fi
  security find-identity -v "$keychain" |
    sed -n 's/^ *[0-9][0-9]*) [0-9A-F]\{40\} "\(.*\)"$/\1/p' |
    grep -E "^($1): .* \\(${team}\\)$" | head -n 1 || true
}

found=""
if [ -n "${APPLE_DEVELOPER_ID_P12:-}" ] && [ -n "${APPLE_DEVELOPER_ID_PASSWORD:-}" ]; then
  import "$APPLE_DEVELOPER_ID_P12" "$APPLE_DEVELOPER_ID_PASSWORD"
  name="$(identity "Developer ID Application")"
  if [ -n "$name" ]; then
    echo "JF_MACOS_SIGN_IDENTITY=$name" >> "$env_file"
    echo "keychain.sh: direct downloads are signed by $name"
    found=yes
  else
    echo "::warning title=No Developer ID::APPLE_DEVELOPER_ID_P12 holds no Developer ID Application identity${APPLE_TEAM_ID:+ of team $APPLE_TEAM_ID}; the direct download stays ad-hoc signed"
  fi
else
  notice "Not signed" "APPLE_DEVELOPER_ID_P12 or APPLE_DEVELOPER_ID_PASSWORD is not set: the direct download is ad-hoc signed and not notarized (docs/store-release.md)"
fi

if [ -n "${APPLE_MAS_APP_P12:-}" ] && [ -n "${APPLE_MAS_INSTALLER_P12:-}" ] && [ -n "${APPLE_MAS_PASSWORD:-}" ]; then
  import "$APPLE_MAS_APP_P12" "$APPLE_MAS_PASSWORD"
  import "$APPLE_MAS_INSTALLER_P12" "$APPLE_MAS_PASSWORD"
  app_name="$(identity "Apple Distribution|3rd Party Mac Developer Application")"
  installer_name="$(identity "3rd Party Mac Developer Installer|Mac Installer Distribution")"
  if [ -n "$app_name" ] && [ -n "$installer_name" ]; then
    echo "JF_MAS_APP_IDENTITY=$app_name" >> "$env_file"
    echo "JF_MAS_INSTALLER_IDENTITY=$installer_name" >> "$env_file"
    echo "keychain.sh: the Mac App Store package is signed by $app_name and $installer_name"
    found=yes
  else
    echo "::warning title=No Mac App Store identities::APPLE_MAS_APP_P12 and APPLE_MAS_INSTALLER_P12 must hold an Apple Distribution and a Mac Installer Distribution identity${APPLE_TEAM_ID:+ of team $APPLE_TEAM_ID}; no Mac App Store package"
  fi
else
  notice "No Mac App Store package" "APPLE_MAS_APP_P12, APPLE_MAS_INSTALLER_P12 or APPLE_MAS_PASSWORD is not set (docs/store-release.md)"
fi

if [ -n "$found" ]; then
  echo "JF_MACOS_KEYCHAIN=$keychain" >> "$env_file"
fi
