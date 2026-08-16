use anyhow::{bail, Context, Result};
use simple_network::security::pqc::{secure_client, secure_server, Identity, SecureConnection};
use simple_network::transport::tcp::TcpTransport;
use simple_network::transport::traits::{Connection, Listener, Transport};
use std::path::Path;

use crate::protocol::{WireMsg, PROTOCOL_VERSION};
use crate::vault::VaultManager;

pub async fn connect_as_client(
    vault_path: &Path,
    password: &[u8],
    peer: &str,
    addr: &str,
) -> Result<SecureConnection> {
    let mut mgr = VaultManager::new(vault_path);
    mgr.create_or_open(password)?;
    let id = mgr.get_or_create_identity()?;
    let peer_vk = mgr.get_pinned_peer(peer)?;
    let transport = TcpTransport;
    let conn = transport.connect(addr).await?;
    secure_client(conn, id, peer_vk).await
}

pub async fn bind_server(
    vault_path: &Path,
    password: &[u8],
    peer: &str,
    addr: &str,
) -> Result<(Box<dyn Listener>, Identity, Vec<u8>)> {
    let mut mgr = VaultManager::new(vault_path);
    mgr.create_or_open(password)?;
    let id = mgr.get_or_create_identity()?;
    let peer_vk = mgr.get_pinned_peer(peer)?;
    let transport = TcpTransport;
    let listener = transport.bind(addr).await?;
    Ok((listener, id, peer_vk))
}

pub async fn handshake_server(
    conn: Box<dyn Connection>,
    id: Identity,
    peer_vk: Vec<u8>,
) -> Result<SecureConnection> {
    secure_server(conn, id, peer_vk).await
}

pub async fn send_msg(conn: &mut SecureConnection, msg: &WireMsg) -> Result<()> {
    let bytes = serde_json::to_vec(msg).context("encode wire msg")?;
    conn.send(&bytes).await?;
    Ok(())
}

pub async fn recv_msg(conn: &mut SecureConnection) -> Result<WireMsg> {
    let bytes = conn.recv().await?;
    let msg: WireMsg = serde_json::from_slice(&bytes).context("decode wire msg")?;
    if let WireMsg::Error { message } = &msg {
        bail!("peer error: {message}");
    }
    Ok(msg)
}

pub async fn expect_ack(conn: &mut SecureConnection) -> Result<()> {
    match recv_msg(conn).await? {
        WireMsg::Ack => Ok(()),
        other => bail!("expected Ack, got {other:?}"),
    }
}

pub async fn client_hello(conn: &mut SecureConnection) -> Result<()> {
    send_msg(
        conn,
        &WireMsg::Hello {
            version: PROTOCOL_VERSION,
        },
    )
    .await?;
    match recv_msg(conn).await? {
        WireMsg::HelloOk { version } if version == PROTOCOL_VERSION => Ok(()),
        WireMsg::HelloOk { version } => bail!("unsupported peer protocol version {version}"),
        other => bail!("expected HelloOk, got {other:?}"),
    }
}

pub async fn server_hello(conn: &mut SecureConnection) -> Result<()> {
    match recv_msg(conn).await? {
        WireMsg::Hello { version } if version == PROTOCOL_VERSION => {
            send_msg(
                conn,
                &WireMsg::HelloOk {
                    version: PROTOCOL_VERSION,
                },
            )
            .await
        }
        WireMsg::Hello { version } => {
            send_msg(
                conn,
                &WireMsg::Error {
                    message: format!("unsupported version {version}"),
                },
            )
            .await?;
            bail!("unsupported client protocol version {version}")
        }
        other => bail!("expected Hello, got {other:?}"),
    }
}

pub async fn connect_with_identity(
    addr: &str,
    id: Identity,
    peer_vk: Vec<u8>,
) -> Result<SecureConnection> {
    let transport = TcpTransport;
    let conn = transport.connect(addr).await?;
    secure_client(conn, id, peer_vk).await
}
