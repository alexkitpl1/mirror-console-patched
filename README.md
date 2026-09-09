# mirror-console (patched)

Small **local-only** console for mirroring and controlling **Android** phones
(via `adb` + `scrcpy`) and **iPhones** (via the open-source
[WebDriverAgent](https://github.com/appium/WebDriverAgent)) from a Mac /
Linux / Windows desktop. No cloud, no accounts, no telemetry — the binary
never contacts a remote server.

This repository is the original bundle **plus four patches** needed to make
it build and drive real devices on modern toolchains — macOS 26, Xcode 26.6,
Appium/WebDriverAgent 8.x. Without them `ios-screenshot` and `ios-tap`
silently return errors and the `.ipa` build fails at the packaging step even
though `xcodebuild archive` succeeds. See [`PATCHES.md`](PATCHES.md) for the
full diff-by-diff explanation.

The upstream project's own README, with its command reference and design
notes, lives at [`mirror-console/README.md`](mirror-console/README.md).

---

## Repository layout

```
.
├── DEPLOY.md                # release/deployment notes from the author
├── PATCHES.md               # what changed in this fork and why
├── .github/workflows/       # macOS CI build
└── mirror-console/          # the tool itself
    ├── README.md            #   original README (commands, design)
    ├── src/                 #   Rust sources (pure std, no crates)
    ├── build-macos.sh       #   universal (arm64+x86_64) build
    ├── build-wda-ipa-macos.sh   #   builds the on-device WDA agent .ipa
    ├── ios-setup-macos.sh
    └── dist/                #   prebuilt Linux/Windows mconsole
```

---

## Quick start (macOS)

Everything below is verified on macOS 26 (Tahoe) + Xcode 26.6 + Apple Silicon.

**1. Toolchain**

```
brew install rustup scrcpy android-platform-tools     # go-ios is NOT in brew
rustup default stable
rustup target add aarch64-apple-darwin x86_64-apple-darwin
```

`go-ios` (needed for the `.ipa` install step) is not in Homebrew — download
the mac binary from
<https://github.com/danielpaulus/go-ios/releases> and put it on `PATH`.

**2. Build the CLI**

```
cd mirror-console
./build-macos.sh           # → dist/mconsole (universal arm64+x86_64)
./dist/mconsole doctor     # should show adb + scrcpy as [ok]
```

**3. Build the on-device iOS agent (WDA .ipa)**

Requires a full Xcode (not just CLT) and an Apple Developer account. Find
your Team ID in `Xcode → Settings → Accounts` (10 chars, e.g. `K7RN84UPN9`).

```
./build-wda-ipa-macos.sh <YOUR_TEAM_ID>
```

The first `codesign` step needs to reach the login keychain. If you see
`errSecInternalComponent`, either:

- run the script once from **Terminal.app** so the "codesign wants to sign
  using key … in login keychain" dialog is reachable — click **"Always
  Allow"** (not "Allow", or the dialog reappears per framework); **or**
- give `codesign` permanent access up front:

  ```
  security set-key-partition-list -S apple-tool:,apple:,codesign: \
    -s ~/Library/Keychains/login.keychain-db
  ```

Output: `dist/WebDriverAgentRunner.ipa` (~6-7 MB).

**4. Install the agent on the iPhone and start it**

Connect the iPhone, unlock it, tap "Trust" on the pairing prompt, then:

```
go-ios install --path=dist/WebDriverAgentRunner.ipa --udid <UDID>
go-ios runwda --udid <UDID>              # keep running in one terminal
go-ios forward 8100 8100 --udid <UDID>   # keep running in another
```

Find the UDID with `go-ios list`.

**5. Drive it**

```
./dist/mconsole ios-status
./dist/mconsole ios-screenshot iphone.png
./dist/mconsole ios-home
./dist/mconsole ios-tap 0.5 0.5
./dist/mconsole ios-swipe 0.5 0.7 0.5 0.3
./dist/mconsole ios-text "hello"
```

Any of the `ios-*` commands can be pointed at a different WDA endpoint with
`MC_WDA_HOST` / `MC_WDA_PORT`.

---

## Quick start (Android)

```
brew install scrcpy android-platform-tools    # or apt/pacman/choco equivalents
./dist/mconsole devices                       # list connected phones
./dist/mconsole mirror <serial>               # or `-` for the only device
./dist/mconsole screenshot <serial> phone.png
./dist/mconsole key <serial> home
```

Full command reference: [`mirror-console/README.md`](mirror-console/README.md).

---

## What this fork changes

Four surgical patches, all documented in [`PATCHES.md`](PATCHES.md):

1. **`src/ios.rs` — `screenshot()`** now accepts the nested
   `{"value":{"screenshot":"…"}}` response WDA 8.x returns (in addition to
   the pre-8 flat `{"value":"…"}` form).
2. **`src/ios.rs` — `tap()`** hits `POST /session/:sid/wda/tap` first
   (Appium 8.x endpoint) with a fallback to the old `/wda/tap/0`.
3. **`build-wda-ipa-macos.sh`** finds the signed `.app` in DerivedData
   `Intermediates.noindex/…/UninstalledProducts/iphoneos/`, because
   `xcodebuild archive` leaves the archive's `Products/` empty for
   `.xctest` UI-testing schemes.
4. **`build-wda-ipa-macos.sh`** — bash `set -u` guard on the empty
   `AUTH_FLAGS` array (`${AUTH_FLAGS[@]+"${AUTH_FLAGS[@]}"}`), otherwise
   the script aborts before `xcodebuild` even starts when you're not using
   an App Store Connect API key.

No behaviour changes for the pre-patch environments — everything falls back
to the old code path if the new one doesn't apply.

---

## Licence

The delivered upstream bundle carries no explicit licence file. This fork
adds no code beyond the four patches above; the patches are trivial and are
released under the same terms as whatever the upstream author chooses. If
that's a problem for your use case, open an issue.
