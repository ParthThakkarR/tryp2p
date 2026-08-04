//! Persistent iroh identity management.
//!
//! On first launch, generates a new Ed25519 SecretKey and saves it to disk.
//! On subsequent launches, loads the existing key so the NodeId stays permanent.

use std::path::{Path, PathBuf};
use p2ptransfer_core::crypto::identity::{derive_secret_key_from_short_id, generate_random_short_id};

/// Load or generate a persistent 8-hex short ID and derive the iroh SecretKey.
pub fn load_or_create_identity(data_dir: &Path) -> anyhow::Result<(String, iroh::SecretKey)> {
    let old_key_path = data_dir.join("identity.key");
    let short_id_path = data_dir.join("short_id.txt");
    let config_path = dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("p2p").join("config.toml");
    let resume_dir = data_dir.join("resume");

    // Migrate/Delete old identity if present, forcing a new deterministic one
    if old_key_path.exists() {
        let _ = std::fs::remove_file(&old_key_path);
    }
    
    // Check if short_id is valid (8 uppercase hex chars). If not, we trigger a cache clear.
    let mut needs_reset = false;
    if short_id_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&short_id_path) {
            let trimmed = content.trim();
            if trimmed.len() != 8 || !trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
                needs_reset = true;
            }
        } else {
            needs_reset = true;
        }
    } else {
        needs_reset = true; // First run with new system
    }

    if needs_reset {
        // Clear all old cache / config for fresh start
        let _ = std::fs::remove_dir_all(&resume_dir);
        let _ = std::fs::remove_file(&config_path);
        let _ = std::fs::remove_file(&short_id_path);
        tracing::info!("Cleared legacy cache and DB to migrate to new 8-char hex system.");
    }

    let short_id = if short_id_path.exists() {
        std::fs::read_to_string(&short_id_path)?.trim().to_uppercase()
    } else {
        std::fs::create_dir_all(data_dir)?;
        let id = generate_random_short_id();
        std::fs::write(&short_id_path, &id)?;
        tracing::info!("Generated new deterministic short ID, saved to {:?}", short_id_path);
        id
    };

    let secret = derive_secret_key_from_short_id(&short_id)?;
    Ok((short_id, secret))
}

pub fn identity_key_path(data_dir: &Path) -> PathBuf {
    data_dir.join("short_id.txt")
}
