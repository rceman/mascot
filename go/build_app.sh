#!/bin/sh
# Build the release binary and assemble MascotGo.app.
set -eu
cd "$(dirname "$0")"
export SDKROOT="${SDKROOT:-/Library/Developer/CommandLineTools/SDKs/MacOSX26.sdk}"
GO="${GO:-$HOME/go/bin/go1.25.3}"
"$GO" build -o /tmp/mascot-go-app .
APP=packaging/MascotGo.app
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp /tmp/mascot-go-app "$APP/Contents/MacOS/mascot"
chmod +x "$APP/Contents/MacOS/mascot"
codesign --force --sign - "$APP" >/dev/null 2>&1 || true
echo "built $APP"
