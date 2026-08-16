//! Remote transfer and PQC pairing for simple-backups.
//!
//! Enable with `--features pqc` (pulls `simple-network` + `simple-secrets`).

#![cfg_attr(not(feature = "pqc"), allow(dead_code))]

#[cfg(feature = "pqc")]
mod pair;
#[cfg(feature = "pqc")]
mod protocol;
#[cfg(feature = "pqc")]
mod pushpull;
#[cfg(feature = "pqc")]
mod session;
#[cfg(feature = "pqc")]
mod vault;

#[cfg(feature = "pqc")]
pub use pair::{generate_pairing_code, pair_peers, PairRole};
#[cfg(feature = "pqc")]
pub use pushpull::{
    pull_from_peer, pull_with_identity, push_to_peer, push_with_identity, serve_peer,
    serve_with_identity_once, TransferStats,
};
#[cfg(feature = "pqc")]
pub use vault::{prompt_password, VaultManager};

/// True when this build includes PQC transfer support.
pub fn pqc_enabled() -> bool {
    cfg!(feature = "pqc")
}
