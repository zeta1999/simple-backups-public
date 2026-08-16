# simple-backups Android companion

Rebranded from `tools/file_sharing/android`. UI talks to [`BackupNative`](app/src/main/java/com/simpletools/backups/BackupNative.kt)
(stub bridge) so the app builds without a Go/Rust AAR.

See [PROTOCOL.md](PROTOCOL.md) for the QR payload and native swap-in plan.

## Build

```bash
./gradlew :app:assembleDebug
```

## Current behaviour

- Local passphrase vault + device fingerprint (stub seed, not ML-DSA yet)
- Pair via QR / paste: `simple-backups:v1:pair:host:port:code`
- Connect / watch service / push buttons drive the stub state machine
- Real PQC push lands when `backups-ffi` replaces the stub
