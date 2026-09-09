# Patches applied on top of the delivered bundle

Four small fixes needed to make `mirror-console` build and drive current iOS
devices on macOS 26 / Xcode 26.6 / WDA 8.x. All four are already in this
tree; this file documents them so a future maintainer knows why the diffs
exist.

## 1. `src/ios.rs:264-273` — `screenshot()` accepts nested WDA response

Modern WDA (both Appium mainline 8.x and the Facebook / GADS fork) return

    { "value": { "screenshot": "<base64>" } }

The delivered code parsed `value` as a flat string, so it printed
`no base64 image in response` on every call. Patch tries the nested
`screenshot` key first, then falls back to the pre-8.x flat `value`.

## 2. `src/ios.rs:202-211` — `tap()` uses the modern endpoint

Appium/WDA 8.x removed the `/wda/tap/0` (root-element context) suffix.
The endpoint is now `POST /session/:sid/wda/tap` with just `{"x":…,"y":…}`.
Patch tries the new form first, falls back to the old one for older forks.

## 3. `build-wda-ipa-macos.sh` — locate signed `.app` in DerivedData

`WebDriverAgentRunner` is a UI-testing target (`.xctest`). For UI-testing
schemes `xcodebuild archive` leaves `xcarchive/Products/` **empty** and
drops the signed `WebDriverAgentRunner-Runner.app` under

    ~/Library/Developer/Xcode/DerivedData/WebDriverAgent-*/Build/Intermediates.noindex/ArchiveIntermediates/WebDriverAgentRunner/IntermediateBuildFilesPath/UninstalledProducts/iphoneos/

The delivered script searched only `xcarchive/Products/` and failed with
`build produced no .app` even though the archive succeeded. Patch reads
`BUILD_DIR` from `xcodebuild -showBuildSettings`, looks in the
UninstalledProducts directory first, and falls back to the archive layout.

## 4. `build-wda-ipa-macos.sh` — `set -u` compatibility for empty array

Line 57 used `"${AUTH_FLAGS[@]}"` inside a `set -u` script. When
`AUTH_FLAGS` is left empty (the interactive-signing path — no App Store
Connect key), bash treats the expansion as an unbound variable and aborts
before `xcodebuild` even starts. Replaced with the idiomatic
`${AUTH_FLAGS[@]+"${AUTH_FLAGS[@]}"}` guard.

## What still needs manual setup

- `brew install rustup` (keg-only) and `rustup default stable` before
  `./build-macos.sh` — the delivered script calls `rustup target add …`
  but brew's `rust` formula does not ship rustup.
- `go-ios` is not in Homebrew; download the mac binary from
  <https://github.com/danielpaulus/go-ios/releases> and put it on PATH.
- `codesign` needs "Always Allow" on the signing key. If the build
  fails with `errSecInternalComponent`, run once from Terminal.app so
  the UI dialog is reachable, or run:

      security set-key-partition-list -S apple-tool:,apple:,codesign: \
        -s ~/Library/Keychains/login.keychain-db

  once and never see the dialog again.
