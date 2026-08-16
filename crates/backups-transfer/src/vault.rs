use anyhow::{anyhow, Result};
use rpassword::prompt_password as ask;
use simple_network::security::pqc::Identity;
use simple_secrets::core::entropy::DefaultEntropySource;
use simple_secrets::core::manager::SecretManager;
use simple_secrets::crypto::vdf_kdf::Argon2Params;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use zeroize::Zeroizing;

pub struct VaultManager {
    manager: SecretManager,
    vault_path: PathBuf,
}

impl VaultManager {
    pub fn new(vault_path: &Path) -> Self {
        Self {
            manager: SecretManager::new(Arc::new(DefaultEntropySource)),
            vault_path: vault_path.to_path_buf(),
        }
    }

    pub fn create_or_open(&mut self, password: &[u8]) -> Result<()> {
        if self.vault_path.exists() {
            self.manager
                .open_vault(&self.vault_path, password, None)
                .map_err(|e| anyhow!("Failed to open vault: {e}"))?;
        } else {
            if let Some(parent) = self.vault_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            self.manager
                .create_vault(
                    &self.vault_path,
                    password,
                    &Argon2Params::default(),
                    100,
                    None,
                )
                .map_err(|e| anyhow!("Failed to create vault: {e}"))?;
        }
        Ok(())
    }

    pub fn get_or_create_identity(&mut self) -> Result<Identity> {
        if !self.manager.is_unlocked() {
            return Err(anyhow!("Vault is locked"));
        }
        let sk_opt = self
            .manager
            .get_secret("identity/signing_key")
            .map_err(|e| anyhow!("Secret error: {e}"))?;
        let vk_opt = self
            .manager
            .get_secret("identity/verifying_key")
            .map_err(|e| anyhow!("Secret error: {e}"))?;

        if let (Some(sk), Some(vk)) = (sk_opt, vk_opt) {
            Identity::from_bytes(&sk, &vk)
        } else {
            let id = Identity::generate()?;
            let (sk, vk) = id.export()?;
            self.manager
                .put_secret("identity/signing_key", &sk)
                .map_err(|e| anyhow!("Failed to store signing key: {e}"))?;
            self.manager
                .put_secret("identity/verifying_key", &vk)
                .map_err(|e| anyhow!("Failed to store verifying key: {e}"))?;
            Ok(id)
        }
    }

    pub fn pin_peer(&mut self, peer_name: &str, peer_vk: &[u8]) -> Result<()> {
        if !self.manager.is_unlocked() {
            return Err(anyhow!("Vault is locked"));
        }
        let key_name = format!("peers/{peer_name}/verifying_key");
        self.manager
            .put_secret(&key_name, peer_vk)
            .map_err(|e| anyhow!("Failed to pin peer key: {e}"))?;
        Ok(())
    }

    pub fn get_pinned_peer(&self, peer_name: &str) -> Result<Vec<u8>> {
        if !self.manager.is_unlocked() {
            return Err(anyhow!("Vault is locked"));
        }
        let key_name = format!("peers/{peer_name}/verifying_key");
        self.manager
            .get_secret(&key_name)
            .map_err(|e| anyhow!("Secret error: {e}"))?
            .ok_or_else(|| anyhow!("Peer '{peer_name}' is not paired/pinned"))
    }
}

/// Resolve vault password from `SB_VAULT_PASSWORD` or an interactive prompt.
pub fn prompt_password(confirm_new: bool) -> Result<Zeroizing<String>> {
    if let Ok(p) = std::env::var("SB_VAULT_PASSWORD") {
        return Ok(Zeroizing::new(p));
    }
    let p = ask("Vault password: ").map_err(|e| anyhow!("{e}"))?;
    if confirm_new {
        let p2 = ask("Confirm password: ").map_err(|e| anyhow!("{e}"))?;
        if p != p2 {
            anyhow::bail!("passwords do not match");
        }
    }
    Ok(Zeroizing::new(p))
}
