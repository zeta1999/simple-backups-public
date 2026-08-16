use crate::vault::VaultManager;
use anyhow::Result;
use simple_network::security::pqc::{pair_exchange, random_secret};
use simple_network::transport::tcp::TcpTransport;
use simple_network::transport::traits::Transport;
use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub enum PairRole {
    /// Connect to peer address.
    Initiator,
    /// Listen on address for peer.
    Listener,
}

pub fn generate_pairing_code() -> Result<String> {
    random_secret()
}

pub async fn pair_peers(
    vault_path: &Path,
    password: &[u8],
    peer_name: &str,
    addr: &str,
    code: &str,
    role: PairRole,
) -> Result<Vec<u8>> {
    let mut mgr = VaultManager::new(vault_path);
    mgr.create_or_open(password)?;
    let id = mgr.get_or_create_identity()?;

    let transport = TcpTransport;
    let peer_vk = match role {
        PairRole::Initiator => {
            let mut conn = transport.connect(addr).await?;
            pair_exchange(&mut conn, &id, code, true).await?
        }
        PairRole::Listener => {
            let mut listener = transport.bind(addr).await?;
            let mut conn = listener.accept().await?;
            pair_exchange(&mut conn, &id, code, false).await?
        }
    };

    mgr.pin_peer(peer_name, &peer_vk)?;
    Ok(peer_vk)
}
