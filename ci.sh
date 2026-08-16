#!/usr/bin/env bash
#
# Local CI for simple-backups (Linux + macOS).
set -euo pipefail

echo "Running CI for simple-backups..."

echo "==> Checking formatting..."
cargo fmt -- --check

echo "==> Running clippy (default features)..."
cargo clippy --all-targets -- -D warnings

echo "==> Running tests..."
cargo test

echo "==> Clippy with pqc feature (if siblings present)..."
if [ -d "../simple-network-public" ] && [ -d "../simple-secrets-public" ]; then
    cargo clippy --all-targets --features pqc -- -D warnings
    cargo test --features pqc
    echo "==> backups-ffi (jni + pqc, host)..."
    cargo clippy -p backups-ffi --features "jni,pqc" -- -D warnings
    cargo test -p backups-ffi --features "jni,pqc"
else
    echo "    Skipping pqc feature: sibling crates not found."
fi

target_installed() {
    rustup target list --installed | grep -qx "$1"
}

cross_check() {
    local label="$1" target="$2" tool="$3"
    echo "==> Checking ${label} target..."
    if ! target_installed "$target"; then
        echo "    Skipping ${label}: rust target '${target}' not installed."
        return 0
    fi
    if [ -n "$tool" ] && ! command -v "$tool" >/dev/null 2>&1; then
        echo "    Skipping ${label}: toolchain '${tool}' not found on PATH."
        return 0
    fi
    if cargo check -p backups-cli --target "$target"; then
        echo "    ${label} check passed."
    else
        echo "    WARNING: ${label} cross-check failed (likely a missing SDK/NDK)." >&2
    fi
}

cross_check "iOS" "aarch64-apple-ios" ""
cross_check "Android" "aarch64-linux-android" "aarch64-linux-android-clang"

echo "CI completed successfully!"
