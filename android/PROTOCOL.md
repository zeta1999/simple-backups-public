# Android ↔ desktop protocol

The phone companion pairs with a desktop `simple-backups` repo over the same
PQC channel used by `serve` / `push` / `pull`.

## QR / manual payload

```text
simple-backups:v1:pair:<host:port>:<onetimecode>
```

Generate on the desktop:

```bash
cargo run -p backups-cli --features pqc -- pair-qr --addr 192.168.1.10:9876
```

Then run the desktop listener with the same code:

```bash
cargo run -p backups-cli --features pqc -- pair \
  --vault vault.bin --peer phone \
  --addr 0.0.0.0:9876 --code "$CODE" --listen
```

On the phone: **Scan / pair desktop** (or long-press for manual paste).

## Sync (target)

After pairing, the native bridge (Rust via uniffi, TBD) should:

1. Open `secure_client` to the pinned peer.
2. Create / update a local content-addressed repo under app storage.
3. `push --latest` semantics for “Push full/incremental”.
4. Optionally `pull` restore paths chosen in Settings.

Today `BackupNative` is a **stub**: it stores passphrase, nickname, peer addr,
and pairing payload, and exposes the UI flow without ML-DSA/ML-KEM yet.

## Bridge replacement plan

1. ~~Expose `backups-ffi` C + JNI ABI~~ (`crates/backups-ffi`, features `jni` / `pqc`).
2. Build Android `.so` with `scripts/build-android-ffi.sh` (needs NDK + `aarch64-linux-android` target).
3. `NativeFfi` loads `libbackups_ffi.so` when present; `BackupNative` falls back to the Kotlin stub.
4. JNI now exposes `identityEnsure`, `pair`, `hasPeer`, `snapshotAndPush` (see `NativeFfi`).
5. Remaining: scan real watch-dirs into the snapshot source instead of the heartbeat file;
   desktop must `serve` after `pair --listen`.
