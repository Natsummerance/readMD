#!/bin/bash
# ============================================================
#  ReadMD Installer (macOS / Linux) - 100% Pure Rust Native Edition
#  - Build native binary with Cargo and install
# ============================================================
set -e
cd "$(dirname "$0")/../.."

echo "[1/3] Checking build environment ..."
if ! command -v cargo &>/dev/null; then
    echo
    echo "Rust toolchain (cargo) not found. Please install Rust:"
    echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    echo
    exit 1
fi

echo "[2/3] Compiling native release binary with Cargo ..."
cargo build --release -p readmd-kernel

echo "[3/3] Setting up executable ..."
mkdir -p dist
cp rust/target/release/readmd ./ReadMD
chmod +x ./ReadMD scripts/run.sh 2>/dev/null || true

echo
echo "Done! Native binary built at: ./ReadMD"
echo "To run ReadMD:"
echo "  ./scripts/run.sh [file.md]  or  ./ReadMD [file.md]"
echo
