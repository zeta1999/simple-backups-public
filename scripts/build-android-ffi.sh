#!/usr/bin/env bash
# Cross-compile libbackups_ffi.so for Android and stage into the app jniLibs.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/android/app/src/main/jniLibs"
API="${ANDROID_API:-26}"

if [[ -z "${ANDROID_NDK_HOME:-}" ]]; then
  echo "ANDROID_NDK_HOME is not set" >&2
  exit 1
fi

build_one() {
  local target="$1" abi="$2" triple="$3"
  local clang="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/"*"/bin/${triple}${API}-clang"
  # shellcheck disable=SC2086
  clang=$(echo $clang)
  if [[ ! -x "$clang" ]]; then
    echo "missing NDK clang for $triple (API $API)" >&2
    exit 1
  fi
  echo "==> $target ($abi)"
  cargo ndk -t "$abi" -o "$OUT" build -p backups-ffi --release --features "jni,pqc" 2>/dev/null \
    || (
      export CC_$target="$clang"
      export AR_$target="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/"*"/bin/llvm-ar"
      AR_$target=$(echo $AR_$target)
      export CARGO_TARGET_$(echo "$target" | tr 'a-z-' 'A-Z_')_LINKER="$clang"
      rustup target add "$target" >/dev/null 2>&1 || true
      cargo build -p backups-ffi --release --target "$target" --features "jni,pqc"
      mkdir -p "$OUT/$abi"
      cp "$ROOT/target/$target/release/libbackups_ffi.so" "$OUT/$abi/"
    )
}

mkdir -p "$OUT"
build_one aarch64-linux-android arm64-v8a aarch64-linux-android
echo "Staged libs under $OUT"
find "$OUT" -name 'libbackups_ffi.so' -print
