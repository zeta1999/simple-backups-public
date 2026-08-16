//! Core types for simple-backups: object IDs, manifests, and job configs.

mod job;
mod manifest;
mod object_id;
mod pair_payload;
mod paths;

pub use job::{expand_path, JobConfig};
pub use manifest::{FileEntry, FileKind, SnapshotManifest, FORMAT_VERSION};
pub use object_id::{hash_bytes, hash_file, ObjectId};
pub use pair_payload::{format_pair_payload, parse_pair_payload, PairPayload, PAIR_PREFIX};
pub use paths::{normalize_rel_path, path_is_safe, symlink_target_is_safe, validate_snapshot_id};
