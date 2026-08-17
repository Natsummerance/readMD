#!/bin/bash
# ============================================================
#  ReadMD - one-click run (venv python)
# ============================================================
set -e
cd "$(dirname "$0")"

if [ ! -f ".venv/bin/python" ]; then
    echo "[ReadMD] Please run install.sh or setup.sh first."
    exit 1
fi

exec ".venv/bin/python" "readmd.py" "$@"
