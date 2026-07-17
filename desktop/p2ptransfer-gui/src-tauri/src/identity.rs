//! Persistent iroh identity management.
//!
//! On first launch, generates a new Ed25519 SecretKey and saves it to disk.
//! On subsequent launches, loads the existing key so the NodeId stays permanent.

use std::path::{Path, PathBuf};

/// Load or generate a persistent iroh SecretKey.
///
/// The key is stored as 32 raw bytes at `data_dir/identity.key`.
/// Returns the SecretKey that should be used for the iroh Endpoint.
pub fn load_or_create_identity(data_dir: &Path) -> anyhow::Result<iroh::SecretKey> {
    let key_path = data_dir.join("identity.key");

    if key_path.exists() {
        let bytes = std::fs::read(&key_path)?;
        if bytes.len() != 32 {
            anyhow::bail!(
                "Identity key file is corrupt (expected 32 bytes, got {}). Delete {:?} to regenerate.",
                bytes.len(),
                key_path
            );
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        let secret = iroh::SecretKey::from_bytes(&arr);
        tracing::info!("Loaded persistent identity from {:?}", key_path);
        Ok(secret)
    } else {
        std::fs::create_dir_all(data_dir)?;
        let secret = iroh::SecretKey::generate();
        std::fs::write(&key_path, secret.to_bytes())?;
        tracing::info!("Generated new identity, saved to {:?}", key_path);
        Ok(secret)
    }
}

/// Returns the path where the identity key is stored.
pub fn identity_key_path(data_dir: &Path) -> PathBuf {
    data_dir.join("identity.key")
}
