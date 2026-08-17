#!/bin/bash
# ============================================================
#  ReadMD Installer (macOS / Linux)
#  - create venv, install deps, generate icon, print instructions
# ============================================================
set -e
cd "$(dirname "$0")"

echo "[1/4] Checking Python ..."
if ! command -v python3 &>/dev/null; then
    echo
    echo "Python 3 not found. Please install Python 3.9+ first:"
    echo "  brew install python3    # macOS (Homebrew)"
    echo "  https://www.python.org/downloads/"
    echo
    exit 1
fi

PY3=$(command -v python3)
echo "  Using: $PY3 ($($PY3 --version 2>&1))"

echo "[2/4] Creating virtual environment and installing dependencies ..."
if [ ! -f ".venv/bin/python" ]; then
    $PY3 -m venv .venv
fi
REQ_FILE="requirements-common.txt"
if [ "$(uname -s)" = "Darwin" ]; then
    REQ_FILE="requirements-macos.txt"
fi
".venv/bin/python" -m pip install --disable-pip-version-check -q -r "$REQ_FILE"

echo "[3/4] Generating icon ..."
if [ -f "tools/make_icon.py" ]; then
    ".venv/bin/python" "tools/make_icon.py" 2>/dev/null || true
fi

echo "[4/4] Done!"
echo
echo "  To run ReadMD:"
echo "    ./run.sh [file.md]"
echo "    or: .venv/bin/python readmd.py [file.md]"
echo
echo "  macOS: To set ReadMD as default .md opener,"
echo "    right-click any .md file → Get Info → Open with → Change All..."
echo
chmod +x run.sh 2>/dev/null || true
