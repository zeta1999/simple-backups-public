# simple-backups — specs

## Goal

Auditable, content-addressed incremental backups with a small CLI, simple YAML
job configs, optional cron/launchd scheduling, and post-quantum peer pairing for
remote replication. Mobile clients (Android first, iOS later) are companions to
the same repository model.

Part of **simple tools**. Reuse sibling crates; do not reinvent crypto.

## Non-goals (v0)

- Deduplicating block chunkers / CDC (file-level CAS is enough for v0)
- Cloud provider SDKs (S3 etc.) — local + paired peer only
- Full GUI — CLI first; Android scaffold only

## Inspiration

- `../tools/incremental_backup` — scan/manifest/incremental/restore/verify UX
- `../simple-network` (`pqc`) — ML-DSA-65 pin + hybrid ML-KEM-768/X25519 channel
- `../simple-remote` — identity vault + `pair` CLI shape
- `../simple-sandbox` — Cargo workspace / `ci.sh` / YAML policy style
- `../tools/file_sharing/android` — Android pairing + foreground sync scaffold

## Repository layout (on disk)

```
<repo>/
  config.yaml          # repo-level defaults
  objects/<ab>/<cdef>  # content-addressed blobs (sha256 hex)
  snapshots/<id>.json  # snapshot manifests
  refs/latest          # points at latest snapshot id
  state/               # local tracker (last scan index), not inside source trees
  vault.bin            # optional: ML-DSA identity + pinned peers (feature `pqc`)
```

Tracker state lives under the repo (or `$XDG_STATE_HOME/simple-backups/`), never
inside the backed-up source by default.

## Snapshot model

- Each snapshot is a manifest of relative paths → object id + metadata.
- Unchanged files reuse prior object ids (no re-write).
- Changed files store a new full object (v0: no text patches yet; CAS already
  dedupes identical content across snapshots).
- Manifests are JSON; `content_hash` covers canonical manifest bytes sans that
  field.
- Snapshot ids are `YYYYMMDD-HHMMSS-<short>` plus optional message.

## Job config (YAML)

```yaml
name: docs
source: ~/Documents
repo: ~/.local/share/simple-backups/repos/docs
exclude:
  - "*.tmp"
  - ".DS_Store"
  - "**/.git/**"
schedule: "0 2 * * *"   # optional cron expression
message: nightly
```

## CLI (v0)

| Command | Purpose |
|---------|---------|
| `init <repo>` | Create empty repository |
| `snapshot` | Create snapshot from source / job file |
| `list` | List snapshots |
| `show <id>` | Show manifest summary |
| `verify [id]` | Verify objects + manifest hashes |
| `restore <id> <target>` | Restore snapshot (optional `--path`) |
| `diff <from> <to>` | Path-level change summary |
| `forget <ids…>` | Delete snapshot manifests (`--gc` optional) |
| `prune --keep-last N` | Drop oldest snapshots (`--gc` / `--dry-run`) |
| `gc` | Delete unreferenced objects |
| `job run <file>` | Run a job config (honours `keep_last` / `gc_after_prune`) |
| `schedule install/uninstall/print` | Emit/install cron or launchd unit |
| `identity-gen` / `pair` / `pair-qr` | PQC identity + out-of-band pairing (`pqc`) |
| `serve` | Accept push/pull from a paired peer (`pqc`) |
| `push` / `pull` | Replicate objects+refs to/from paired peer (`pqc`) |

### Transfer wire protocol (v1)

Over `simple_network::security::pqc::SecureConnection`:

1. `Hello` / `HelloOk` (version = 1)
2. **Push:** `PushBegin{snapshots[{id,content_hash,object_ids}]}` → `WantObjects` →
   `ObjectMeta` + chunked raw bytes → `PushManifest`/`Ack`* → `PushEnd`/`Ack`
3. **Pull:** `PullBegin{snapshot}` → `PullManifest` → `WantObjects` →
   `ObjectMeta` + chunks → `PullEnd`/`Ack`

Object payloads are chunked at 4 MiB to stay under the TCP frame cap.

## PQC pairing

Reuse `simple_network::security::pqc`:

1. Generate durable ML-DSA-65 identity (stored in vault via `simple-secrets`).
2. Out-of-band one-time 128-bit code (hex / QR).
3. `pair_exchange` pins peer verifying keys.
4. Later transfer sessions: hybrid ML-KEM-768 + X25519, ML-DSA auth,
   XChaCha20-Poly1305 records (via `simple-network`).

Never put vault passwords on the default CLI flags in docs/examples; prompt or
env (`SB_VAULT_PASSWORD`).

## Scheduling

- Linux: print crontab line / systemd timer unit.
- macOS: print/install LaunchAgent plist calling `simple-backups job run`.
- CLI must be idempotent and non-interactive for scheduled runs.

## Mobile

- Android: adapt `../tools/file_sharing/android` (pair + scheduled sync).
- iOS: stub app shell; emulator testing later.
- Both speak the same repo protocol once transfer lands.

## Success criteria (v0)

1. Local init → snapshot → list → restore → verify round-trip with tests.
2. Incremental second snapshot stores only changed objects.
3. YAML job + `schedule print` works.
4. With `--features pqc`, identity-gen + pair compile against sibling crates.
5. Loopback push/pull integration test green with `--features pqc`.
6. `./ci.sh` green on host (fmt, clippy `-D warnings`, test).
