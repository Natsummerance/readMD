#!/bin/bash
# ============================================================
#  ReadMD - one-click run (native Rust binary)
# ============================================================
set -e
cd "$(dirname "$0")/.."

if [ -f "./ReadMD" ]; then
    exec "./ReadMD" "$@"
elif [ -f "./rust/target/release/readmd" ]; then
    exec "./rust/target/release/readmd" "$@"
fi

echo "[ReadMD] Native binary not found. Please build with cargo build --release."
exit 1
