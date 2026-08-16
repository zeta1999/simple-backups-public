//! Backup engine: scan sources, create snapshots, restore, verify, diff.

mod exclude;
mod forget;
mod gc;
mod restore;
mod scan;
mod snapshot;
mod verify;

pub use exclude::ExcludeSet;
pub use forget::{forget_snapshots, prune_keep_last, ForgetReport};
pub use gc::{gc_repo, GcReport};
pub use restore::{restore_snapshot, RestoreOptions};
pub use scan::{scan_source, ScanEntry};
pub use snapshot::{create_snapshot, SnapshotOptions, SnapshotStats};
pub use verify::{verify_repo, verify_snapshot, VerifyReport};
