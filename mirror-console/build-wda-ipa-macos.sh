#!/usr/bin/env bash
# Build the iPhone-side agent as an installable .ipa — using the open-source
# WebDriverAgent (Appium). This is the on-device component that mconsole's
# `ios-*` commands talk to. It is the license-free implementation of the same
# XCTest event-injection technique; no proprietary tap agent is used.
#
# MUST run on macOS with Xcode installed and an Apple Developer account
# (free personal team works for 7-day sideload; paid team for longer). A signed
# iOS binary cannot be produced anywhere but a Mac with your signing identity —
# exactly like the original installer, which signed on the user's machine.
#
# Usage:
#   ./build-wda-ipa-macos.sh <APPLE_TEAM_ID> [WDA_DIR]
#
# Find your Team ID:  Xcode > Settings > Accounts, or
#   security find-identity -v -p codesigning
set -euo pipefail

TEAM_ID="${1:-}"
WDA_DIR="${2:-$PWD/third_party/WebDriverAgent}"
OUT_DIR="$PWD/dist"
mkdir -p "$OUT_DIR"

if [ -z "$TEAM_ID" ]; then
  echo "usage: ./build-wda-ipa-macos.sh <APPLE_TEAM_ID> [WDA_DIR]"
  echo "  (Xcode > Settings > Accounts to find your Team ID)"
  exit 1
fi

if ! xcode-select -p >/dev/null 2>&1; then
  echo "Xcode command line tools not found. Install Xcode from the App Store first."
  exit 1
fi

if [ ! -d "$WDA_DIR" ]; then
  echo "==> fetching open-source WebDriverAgent"
  git clone --depth 1 https://github.com/appium/WebDriverAgent "$WDA_DIR"
fi

cd "$WDA_DIR"

# In CI, pass an App Store Connect API key so xcodebuild can manage signing
# non-interactively. Set ASC_KEY_ID, ASC_ISSUER_ID and ASC_KEY_PATH (path to the
# .p8). Left empty, xcodebuild uses the Mac's interactive Xcode account.
AUTH_FLAGS=()
if [ -n "${ASC_KEY_ID:-}" ] && [ -n "${ASC_ISSUER_ID:-}" ] && [ -n "${ASC_KEY_PATH:-}" ]; then
  echo "==> using App Store Connect API key for signing"
  AUTH_FLAGS=(-authenticationKeyID "$ASC_KEY_ID" -authenticationKeyIssuerID "$ASC_ISSUER_ID" -authenticationKeyPath "$ASC_KEY_PATH")
fi

echo "==> building & archiving WebDriverAgentRunner for a real device (Team $TEAM_ID)"
xcodebuild \
  -project WebDriverAgent.xcodeproj \
  -scheme WebDriverAgentRunner \
  -destination 'generic/platform=iOS' \
  -allowProvisioningUpdates \
  ${AUTH_FLAGS[@]+"${AUTH_FLAGS[@]}"} \
  DEVELOPMENT_TEAM="$TEAM_ID" \
  CODE_SIGN_STYLE=Automatic \
  -archivePath "$OUT_DIR/WebDriverAgentRunner.xcarchive" \
  archive

# Package the runner .app into a plain .ipa (Payload/ layout).
# WebDriverAgentRunner is a XCTest UI-testing target (.xctest), and for those
# `xcodebuild archive` leaves `xcarchive/Products/` empty and drops the signed
# runner .app in DerivedData/.../UninstalledProducts/iphoneos/. Look there
# first, fall back to the classic archive layout for regular apps.
DERIVED_DATA="$(xcodebuild -project WebDriverAgent.xcodeproj -showBuildSettings 2>/dev/null | awk -F' = ' '/BUILD_DIR /{print $2; exit}')"
APP_PATH=$(/usr/bin/find "${DERIVED_DATA:-/dev/null}/../Intermediates.noindex/ArchiveIntermediates/WebDriverAgentRunner/IntermediateBuildFilesPath/UninstalledProducts" -name 'WebDriverAgentRunner-Runner.app' -type d 2>/dev/null | head -1)
if [ -z "$APP_PATH" ]; then
  APP_PATH=$(/usr/bin/find "$OUT_DIR/WebDriverAgentRunner.xcarchive/Products" -name '*.app' -maxdepth 3 2>/dev/null | head -1)
fi
if [ -z "$APP_PATH" ]; then
  echo "build produced no .app — check the xcodebuild log above"; exit 1
fi

echo "==> packaging .ipa from $APP_PATH"
rm -rf "$OUT_DIR/Payload" "$OUT_DIR/WebDriverAgentRunner.ipa"
mkdir -p "$OUT_DIR/Payload"
cp -R "$APP_PATH" "$OUT_DIR/Payload/"
( cd "$OUT_DIR" && /usr/bin/zip -qr WebDriverAgentRunner.ipa Payload && rm -rf Payload )

echo
echo "==> done: $OUT_DIR/WebDriverAgentRunner.ipa"
echo
echo "Install it on the connected iPhone and start the agent with go-ios:"
echo "  go-ios install --path=$OUT_DIR/WebDriverAgentRunner.ipa --udid <UDID>"
echo "  go-ios runwda --udid <UDID>"
echo "  go-ios forward 8100 8100 --udid <UDID>"
echo "Then drive it:  ./dist/mconsole ios-status"
