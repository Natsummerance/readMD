#!/bin/bash
# ============================================================
#  ReadMD macOS Setup - 100% Pure Rust Native Edition
#  - Build native binary + package macOS .app bundle
# ============================================================
set -e
cd "$(dirname "$0")/../.."

if [ "$(uname -s)" != "Darwin" ]; then
    echo "setup.sh builds a macOS .app and must run on macOS."
    echo "On Linux, use ./scripts/unix/install.sh then ./scripts/run.sh instead."
    exit 1
fi

echo "[1/4] Checking build environment ..."
if ! command -v cargo &>/dev/null; then
    echo "Rust toolchain (cargo) not found. Install first: brew install rust"
    exit 1
fi

echo "[2/4] Compiling native release binary with Cargo ..."
cargo build --release -p readmd-kernel

echo "[3/4] Packaging ReadMD.app bundle ..."
APP_PATH="dist/ReadMD.app"
mkdir -p "$APP_PATH/Contents/MacOS"
mkdir -p "$APP_PATH/Contents/Resources"

cp rust/target/release/readmd "$APP_PATH/Contents/MacOS/ReadMD"
chmod +x "$APP_PATH/Contents/MacOS/ReadMD"

# Copy icons and resources if present
if [ -f "assets/readmd.icns" ]; then
    cp assets/readmd.icns "$APP_PATH/Contents/Resources/"
fi
cp -r assets "$APP_PATH/Contents/Resources/" 2>/dev/null || true

cat <<'PLIST' > "$APP_PATH/Contents/Info.plist"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>ReadMD</string>
    <key>CFBundleDisplayName</key>
    <string>ReadMD</string>
    <key>CFBundleIdentifier</key>
    <string>asia.readmd.desktop</string>
    <key>CFBundleVersion</key>
    <string>0.0.1</string>
    <key>CFBundleShortVersionString</key>
    <string>0.0.1</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleExecutable</key>
    <string>ReadMD</string>
    <key>CFBundleIconFile</key>
    <string>readmd.icns</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.15</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
PLIST

echo "[4/4] Done! Built: $APP_PATH"
echo
echo "  To run from source:  ./scripts/run.sh [file.md]"
echo "  To run packaged:     open \"$APP_PATH\""
echo "  To install system-wide: cp -r \"$APP_PATH\" /Applications/"
echo
chmod +x scripts/run.sh scripts/unix/install.sh scripts/unix/setup.sh 2>/dev/null || true
