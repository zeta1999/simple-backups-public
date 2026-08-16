#![cfg(feature = "pqc")]

use backups_engine::{create_snapshot, SnapshotOptions};
use backups_store::Repository;
use backups_transfer::{pull_with_identity, push_with_identity, serve_with_identity_once};
use simple_network::security::pqc::Identity;
use simple_network::transport::tcp::TcpTransport;
use simple_network::transport::traits::Transport;
use std::fs;
use std::time::Duration;

async fn push_with_retry(
    repo: &Repository,
    addr: &str,
    id: Identity,
    peer_vk: Vec<u8>,
) -> anyhow::Result<backups_transfer::TransferStats> {
    let mut last = None;
    for _ in 0..50 {
        // Identity is moved on success; reload from export for retries.
        match id.export() {
            Ok((sk, vk)) => {
                let fresh = Identity::from_bytes(&sk, &vk)?;
                match push_with_identity(repo, addr, fresh, peer_vk.clone()).await {
                    Ok(s) => return Ok(s),
                    Err(e) => {
                        last = Some(e);
                        tokio::time::sleep(Duration::from_millis(20)).await;
                    }
                }
            }
            Err(e) => return Err(e),
        }
    }
    Err(last.unwrap_or_else(|| anyhow::anyhow!("push retries exhausted")))
}

#[tokio::test]
async fn push_then_pull_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    let repo_a = tmp.path().join("repo-a");
    let repo_b = tmp.path().join("repo-b");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("hello.txt"), b"hello pqc").unwrap();
    fs::write(src.join("note.md"), b"notes").unwrap();

    let a = Repository::init(&repo_a, Some("a".into())).unwrap();
    create_snapshot(
        &a,
        &src,
        &SnapshotOptions {
            message: Some("one".into()),
            ..Default::default()
        },
    )
    .unwrap();

    let b = Repository::init(&repo_b, Some("b".into())).unwrap();

    let alice = Identity::generate().unwrap();
    let bob = Identity::generate().unwrap();
    let alice_vk = alice.verifying_key();
    let bob_vk = bob.verifying_key();
    let (alice_sk, alice_vk_bytes) = alice.export().unwrap();

    let transport = TcpTransport;
    let mut listener = transport.bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();

    let server = tokio::spawn(async move {
        serve_with_identity_once(&b, &mut listener, bob, alice_vk)
            .await
            .expect("serve")
    });

    let alice_client = Identity::from_bytes(&alice_sk, &alice_vk_bytes).unwrap();
    let push_stats = push_with_retry(&a, &addr, alice_client, bob_vk)
        .await
        .expect("push");
    assert!(push_stats.snapshots >= 1);
    assert!(push_stats.objects_sent >= 2);

    let serve_stats = server.await.unwrap();
    assert_eq!(serve_stats.snapshots, push_stats.snapshots);
    assert_eq!(serve_stats.objects_received, push_stats.objects_sent);

    // Pull into a fresh repo from B.
    let repo_c = tmp.path().join("repo-c");
    let c = Repository::init(&repo_c, Some("c".into())).unwrap();
    let b2 = Repository::open(&repo_b).unwrap();

    let alice2 = Identity::generate().unwrap();
    let bob2 = Identity::generate().unwrap();
    let alice_vk2 = alice2.verifying_key();
    let bob_vk2 = bob2.verifying_key();
    let (alice2_sk, alice2_vk) = alice2.export().unwrap();

    let mut listener = transport.bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let server = tokio::spawn(async move {
        serve_with_identity_once(&b2, &mut listener, bob2, alice_vk2)
            .await
            .expect("serve pull")
    });

    let mut last = None;
    let pull_stats = {
        let mut stats = None;
        for _ in 0..50 {
            let client = Identity::from_bytes(&alice2_sk, &alice2_vk).unwrap();
            match pull_with_identity(&c, &addr, client, bob_vk2.clone(), "latest").await {
                Ok(s) => {
                    stats = Some(s);
                    break;
                }
                Err(e) => {
                    last = Some(e);
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            }
        }
        stats.ok_or_else(|| last.unwrap()).expect("pull")
    };
    let _ = server.await.unwrap();
    assert_eq!(pull_stats.snapshots, 1);

    let latest = c.load_snapshot("latest").unwrap();
    assert_eq!(latest.file_count(), 2);
}
