#!/bin/sh
# Build the release binary and assemble MascotRust.app.
set -eu
cd "$(dirname "$0")"
export SDKROOT="${SDKROOT:-/Library/Developer/CommandLineTools/SDKs/MacOSX26.sdk}"
cargo build --release --target aarch64-apple-darwin
APP=packaging/MascotRust.app
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/aarch64-apple-darwin/release/mascot "$APP/Contents/MacOS/mascot"
chmod +x "$APP/Contents/MacOS/mascot"
codesign --force --sign - "$APP" >/dev/null 2>&1 || true
echo "built $APP"
