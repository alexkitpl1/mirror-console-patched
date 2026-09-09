#!/usr/bin/env bash
# Build mirror-console on macOS as a universal (Apple Silicon + Intel) binary.
# Run this ON A MAC with Rust installed (https://rustup.rs). Nothing here needs
# the network at build time beyond fetching the std targets once.
set -euo pipefail
cd "$(dirname "$0")"

echo "==> ensuring rust targets"
rustup target add aarch64-apple-darwin x86_64-apple-darwin

echo "==> building arm64"
cargo build --release --target aarch64-apple-darwin
echo "==> building x86_64"
cargo build --release --target x86_64-apple-darwin

mkdir -p dist
echo "==> lipo -> universal binary"
lipo -create -output dist/mconsole \
  target/aarch64-apple-darwin/release/mconsole \
  target/x86_64-apple-darwin/release/mconsole
chmod +x dist/mconsole

echo "==> done: dist/mconsole"
file dist/mconsole
./dist/mconsole version
echo
echo "Next: install the open-source tools it drives, e.g.  brew install scrcpy android-platform-tools"
echo "Then: ./dist/mconsole doctor   (should show both as [ok])"
