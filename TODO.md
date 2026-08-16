# Handoff TODO (continue on next machine)

## Done here
- [x] Rust workspace: core / store / engine / transfer / ffi / cli
- [x] PQC pair + push/pull (`--features pqc`) over simple-network
- [x] forget / prune / gc
- [x] Android companion UI (`com.simpletools.backups`) with stub FFI
- [x] Debug APK: `dist/simple-backups-debug.apk` (stub native; no `libbackups_ffi.so`)
- [x] iOS stub + SPECS / README / `ci.sh`

## Next
- [x] Confine snapshot ids + restore symlink/mode; cap pull objects to advertised set
- [ ] Install Android NDK; run `scripts/build-android-ffi.sh` and ship `libbackups_ffi.so` in the APK (real identity / pair / push)
- [ ] End-to-end phone ↔ desktop: `pair --listen --advertise` → `serve --peer phone` → phone Scan → Connect → Push
- [ ] Wire watch service to media folders → local snapshot staging → push (replace stub heartbeat)
- [ ] Drop leftover Go bridge crumbs under `android/app/libs/` if still unused
- [ ] Flesh out iOS beyond stub (optional)
- [ ] Cursor webhook remote-control automation (only if asked again)
- [x] Run tests and CI on **linux/arm64** and **linux/amd64** via Docker on Mac (QEMU for amd64)

## Quick start on new machine
```bash
# siblings expected beside this repo for pqc:
#   simple-network, simple-secrets, rust-secure-memory
./ci.sh
cargo build -p backups-cli --features pqc
adb install -r dist/simple-backups-debug.apk
```

Android rebuild without native FFI:
```bash
cd android && ./gradlew :app:assembleDebug
```
