use anyhow::Result;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
use crate::network::wan_constants::BinaryPin;

pub async fn ensure_binary(pin: &BinaryPin, dest_dir: &Path) -> Result<PathBuf> {
    let final_path = dest_dir.join(pin.filename);
    
    if final_path.exists() {
        if verify_checksum(&final_path, pin.sha256).await? {
            return process_post_download(&final_path, dest_dir).await;
        } else {
            tracing::warn!("Checksum mismatch for {}, re-downloading...", pin.filename);
            let _ = tokio::fs::remove_file(&final_path).await;
        }
    }

    if let Some(parent) = final_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    tracing::info!("Downloading {} from {}", pin.filename, pin.url);
    let tmp_path = dest_dir.join(format!("{}.tmp", pin.filename));
    
    let mut response = reqwest::get(pin.url).await?.error_for_status()?;
    let mut file = tokio::fs::File::create(&tmp_path).await?;
    let mut hasher = Sha256::new();

    while let Some(chunk) = response.chunk().await? {
        hasher.update(&chunk);
        file.write_all(&chunk).await?;
    }
    file.flush().await?;
    drop(file);

    let hash_result = hex::encode(hasher.finalize());
    if hash_result != pin.sha256 && pin.sha256 != "0000000000000000000000000000000000000000000000000000000000000000" {
        let _ = tokio::fs::remove_file(&tmp_path).await;
        anyhow::bail!("Checksum mismatch for {}: expected {}, got {}", pin.filename, pin.sha256, hash_result);
    }

    tokio::fs::rename(&tmp_path, &final_path).await?;
    
    process_post_download(&final_path, dest_dir).await
}

async fn verify_checksum(path: &Path, expected: &str) -> Result<bool> {
    if expected == "0000000000000000000000000000000000000000000000000000000000000000" {
        return Ok(true);
    }
    let data = tokio::fs::read(path).await?;
    let hash = hex::encode(Sha256::digest(&data));
    Ok(hash == expected)
}

async fn process_post_download(path: &Path, dest_dir: &Path) -> Result<PathBuf> {
    let mut final_executable = path.to_path_buf();

    // If it's a .tgz file (like cloudflared on macOS), extract it
    if path.extension().map(|s| s == "tgz").unwrap_or(false) {
        tracing::info!("Extracting {}...", path.display());
        let status = tokio::process::Command::new("tar")
            .arg("-xzf")
            .arg(path)
            .arg("-C")
            .arg(dest_dir)
            .status()
            .await?;
        if !status.success() {
            anyhow::bail!("Failed to extract {}", path.display());
        }
        // The cloudflared archive contains a binary named "cloudflared"
        final_executable = dest_dir.join("cloudflared");
    }

    // Make executable if Linux/macOS
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = tokio::fs::metadata(&final_executable).await?.permissions();
        perms.set_mode(0o755);
        tokio::fs::set_permissions(&final_executable, perms).await?;
    }

    // Strip quarantine if macOS
    #[cfg(target_os = "macos")]
    {
        let _ = tokio::process::Command::new("xattr")
            .arg("-d")
            .arg("com.apple.quarantine")
            .arg(&final_executable)
            .status()
            .await;
    }

    Ok(final_executable)
}
