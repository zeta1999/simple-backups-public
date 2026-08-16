//! Wire protocol for repository replication over a PQC `SecureConnection`.
//!
//! Control messages are JSON (`WireMsg`). Object payloads are sent as a
//! following raw record after `ObjectMeta { id, len }`.

use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotOffer {
    pub id: String,
    pub content_hash: String,
    /// Object ids referenced by this snapshot (sha256 hex).
    pub object_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WireMsg {
    Hello {
        version: u32,
    },
    HelloOk {
        version: u32,
    },
    Error {
        message: String,
    },
    Ack,

    /// Client → server: offer snapshots we intend to push.
    PushBegin {
        snapshots: Vec<SnapshotOffer>,
    },
    /// Server → client: object ids still needed.
    WantObjects {
        ids: Vec<String>,
    },
    /// Either side → peer: next record is `len` raw object bytes for `id`.
    ObjectMeta {
        id: String,
        len: u64,
    },
    /// Client → server: full snapshot manifest JSON.
    PushManifest {
        id: String,
        json: String,
    },
    PushEnd,

    /// Client → server: request a snapshot (`"latest"` allowed).
    PullBegin {
        snapshot: String,
    },
    /// Server → client: manifest + object id list for the requested snapshot.
    PullManifest {
        id: String,
        json: String,
        object_ids: Vec<String>,
    },
    PullEnd,
}
