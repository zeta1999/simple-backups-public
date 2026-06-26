# simple-backups

> **Status: coming soon.**

Simple, verifiable backups: content-addressed snapshots an agent can create and restore as part of a safe edit loop.

Part of [**simple tools**](https://zeta1999.github.io/renoir42/simple-tools.html) — small, composable Rust libraries for building tooling fast from a harness.

## Idea

Before a harness lets a model change things, take a snapshot it can roll back to. Content-addressed so snapshots dedupe and verify, simple enough that the create/restore path is auditable.

## License

MIT OR Apache-2.0
