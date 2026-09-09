#!/usr/bin/env bash
# Stand up iPhone control on a Mac using open-source tools, then forward the
# WebDriverAgent port so `mconsole ios-*` can reach it.
#
# One-time prerequisites (installed by this script where possible):
#   - Xcode + command line tools
#   - go-ios          (device tunnel / port forward)  -> brew install go-ios
#   - WebDriverAgent  (Appium's, built once in Xcode) -> https://github.com/appium/WebDriverAgent
#
# WDA is the open-source implementation of the same XCTest event-injection
# technique commercial mirrors use. We do not ship anyone's tap agent.
set -euo pipefail

UDID="${1:-}"
PORT="${MC_WDA_PORT:-8100}"

if ! command -v go-ios >/dev/null 2>&1; then
  echo "==> installing go-ios"
  brew install go-ios || { echo "install go-ios manually: https://github.com/danielpaulus/go-ios"; exit 1; }
fi

if [ -z "$UDID" ]; then
  echo "==> attached devices:"
  go-ios list || true
  echo
  echo "usage: ./ios-setup-macos.sh <UDID>"
  echo "(copy a UDID from the list above)"
  exit 0
fi

echo "==> device info"
go-ios info --udid "$UDID" | sed -n '1,6p' || true

cat <<EOF

Next steps (once WebDriverAgent is built for this device in Xcode):

  1. Start WDA on the device (keeps running):
       go-ios runwda --udid $UDID \\
         --bundleid=com.facebook.WebDriverAgentRunner.xctrunner \\
         --testrunnerbundleid=com.facebook.WebDriverAgentRunner.xctrunner \\
         --xctestconfig=WebDriverAgentRunner.xctest

  2. In another terminal, forward the WDA port to localhost:
       go-ios forward $PORT $PORT --udid $UDID

  3. Verify from mirror-console:
       MC_WDA_PORT=$PORT ./dist/mconsole ios-status
       ./dist/mconsole ios-home
       ./dist/mconsole ios-tap 0.5 0.5
       ./dist/mconsole ios-screenshot iphone.png

All local — nothing leaves your Mac / LAN.
EOF
