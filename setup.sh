#!/bin/bash
# ============================================================
#  ReadMD macOS Setup
#  - install deps + package an unsigned .app bundle
# ============================================================
set -e
cd "$(dirname "$0")"

if [ "$(uname -s)" != "Darwin" ]; then
    echo "setup.sh builds a macOS .app and must run on macOS."
    echo "On Linux, use install.sh then ./run.sh instead."
    exit 1
fi

echo "[1/6] Checking Python ..."
if ! command -v python3 &>/dev/null; then
    echo "Python 3 not found. Install first: brew install python3"
    exit 1
fi
PY3=$(command -v python3)
echo "  Using: $PY3 ($($PY3 --version 2>&1))"

echo "[2/6] Creating venv and installing dependencies ..."
if [ ! -f ".venv/bin/python" ]; then
    $PY3 -m venv .venv
fi
".venv/bin/python" -m pip install --disable-pip-version-check -q -r requirements-test-macos.txt

echo "[3/6] Generating icon ..."
if [ -f "tools/make_icon.py" ]; then
    ".venv/bin/python" "tools/make_icon.py" 2>/dev/null || true
fi

echo "[4/6] Packaging ReadMD (.app bundle) ..."
".venv/bin/python" -m PyInstaller --noconfirm --clean ReadMD-macOS.spec

echo "[5/6] Preparing .app ..."
APP_PATH="dist/ReadMD.app"
if [ -d "$APP_PATH" ]; then
    echo "  Built: $APP_PATH"
    echo "  Move to /Applications for system-wide access:"
    echo "    cp -r \"$APP_PATH\" /Applications/"
else
    echo "  Warning: .app bundle not found at $APP_PATH"
    echo "  Check dist/ for the build output."
fi

echo "[6/6] Done!"
echo
echo "  To run from source:  ./run.sh [file.md]"
echo "  To run packaged:     open \"$APP_PATH\""
echo "  This app is unsigned. On first launch, Control-click it and choose Open."
echo
echo "  macOS: To set ReadMD as default .md opener,"
echo "    right-click any .md → Get Info → Open with → Change All..."
echo
chmod +x run.sh install.sh setup.sh 2>/dev/null || true
