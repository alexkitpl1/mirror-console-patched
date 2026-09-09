# mirror-console

A small, **local-only** console for mirroring and controlling **Android** devices
from a desktop. It is an original tool that drives the open-source
[`scrcpy`](https://github.com/Genymobile/scrcpy) (Apache-2.0) and `adb` (Android
platform-tools, Apache-2.0), which you install separately.

**No cloud, no accounts, no licensing, no telemetry.** Nothing in this program
contacts a remote server. Device notes and stream settings are kept in a small
local text file.

## Build

Requires [Rust](https://rustup.rs). The code is pure `std` — no third-party crates.

- **macOS (primary):** `./build-macos.sh` → produces a universal `dist/mconsole`
  (Apple Silicon + Intel). Must be run on a Mac.
- **Linux:** `cargo build --release` → `target/release/mconsole`
- **Windows:** `cargo build --release` on Windows, or cross-compile from Linux
  with `cargo build --release --target x86_64-pc-windows-gnu` (needs `mingw-w64`).

Prebuilt Linux and Windows binaries are in `dist/`. macOS you build locally with
the script above (an Apple SDK is required, so it cannot be cross-built).

## Prerequisites at runtime

Install the tools it drives:

- macOS: `brew install scrcpy android-platform-tools`
- Linux: `apt install scrcpy adb` (or your distro's packages)
- Windows: install scrcpy (bundles adb) and put it on `PATH`

Then check:

```
mconsole doctor
```

Override discovery with `MC_ADB` / `MC_SCRCPY` if the tools are not on `PATH`.

## Usage

```
mconsole devices                    # list attached devices
mconsole mirror                     # mirror the (single) attached device
mconsole mirror <serial>            # mirror a specific device
mconsole connect 192.168.1.50       # adb-over-Wi-Fi (default port 5555)
mconsole apps -                     # list third-party packages on default device
mconsole install - app.apk          # install an APK
mconsole launch - com.example.app   # launch an app
mconsole stop - com.example.app     # force-stop an app
mconsole key - home                 # home|back|recent|power|volup|voldown|menu|enter
mconsole text - "hello world"       # type text
mconsole screenshot - shot.png      # save a PNG screenshot
mconsole set video_max_size 1280    # stream tuning used by `mirror`
mconsole set video_fps 60
mconsole config                     # show stored settings/notes
```

Use `-` as the serial to target the only attached device.

## iOS (iPhone) control

iPhone control is built on the open-source
[WebDriverAgent](https://github.com/appium/WebDriverAgent) (WDA) — the same
XCTest event-injection technique commercial mirrors use, but license-free. This
project ships an original WDA *client*; it does not include anyone's tap agent.

Setup on the Mac (where the phones are attached):

```
./ios-setup-macos.sh                 # lists UDIDs
./ios-setup-macos.sh <UDID>          # prints the runwda / forward steps
```

### Building the on-device agent (.ipa)

The iPhone-side agent is the open-source WebDriverAgentRunner. Build it into an
installable `.ipa` **on your Mac** (needs Xcode + your Apple Team ID — an iOS
binary cannot be produced or signed anywhere else):

```
./build-wda-ipa-macos.sh <APPLE_TEAM_ID>
# -> dist/WebDriverAgentRunner.ipa
go-ios install --path=dist/WebDriverAgentRunner.ipa --udid <UDID>
```

`ios-setup-macos.sh` then runs and forwards it. Once running:

```
mconsole ios-status                  # confirm WDA is reachable
mconsole ios-home
mconsole ios-tap 0.5 0.5             # normalized 0..1 coordinates
mconsole ios-swipe 0.5 0.8 0.5 0.2  # swipe up
mconsole ios-key back               # edge-swipe back; also: home | switcher
mconsole ios-text "hello"
mconsole ios-screenshot iphone.png
```

Point at a different tunnel with `MC_WDA_HOST` / `MC_WDA_PORT`. For live screen
video, use QuickTime (USB) or [`uxplay`](https://github.com/FDH2/UxPlay)
(AirPlay). Everything stays on your Mac / LAN — no cloud.

Note: bulk photo-import into the camera roll is intentionally not replicated
(the original did it with a custom in-app agent); use Finder/`go-ios` file
transfer or AirDrop instead.

## CI builds (no Mac needed by you)

`.github/workflows/macos-build.yml` runs on a GitHub macOS runner:

- **macOS binary** — builds automatically on push; download it from the run's
  *Artifacts* (`mconsole-macos-universal`).
- **iPhone `.ipa`** — opt-in: Actions tab → *Run workflow* → `build_ipa = true`.
  Requires these repo secrets (Settings → Secrets → Actions):
  - `APPLE_TEAM_ID` — your 10-char Apple Team ID
  - `ASC_KEY_ID`, `ASC_ISSUER_ID` — App Store Connect API key id / issuer id
  - `ASC_API_KEY_BASE64` — the `.p8` key, base64-encoded
    (`base64 -i AuthKey_XXXX.p8`)

  The workflow uses the API key so Xcode can sign non-interactively; the key is
  written to a temp file and deleted after the build.

## License

Apache-2.0. Bundled/driven tools (`adb`, `scrcpy`) keep their own licenses.
