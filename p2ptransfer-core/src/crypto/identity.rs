use anyhow::Result;
use iroh::SecretKey;

/// Deterministically derives a 32-byte Iroh SecretKey from an 8-character string.
pub fn derive_secret_key_from_short_id(short_id: &str) -> Result<SecretKey> {
    // We enforce uppercase internally to prevent mismatch errors.
    let normalized = short_id.trim().to_uppercase();
    if normalized.len() != 8 {
        anyhow::bail!("Short ID must be exactly 8 characters.");
    }
    if !normalized.chars().all(|c| c.is_ascii_hexdigit()) {
        anyhow::bail!("Short ID must contain only hex characters (0-9, A-F).");
    }
    
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"tryp2p_deterministic_key_v1"); // domain separation salt
    hasher.update(normalized.as_bytes());
    
    let hash_bytes: [u8; 32] = hasher.finalize().into();
    Ok(SecretKey::from_bytes(&hash_bytes))
}

/// Generates a random 8-character uppercase hex string.
pub fn generate_random_short_id() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let chars: Vec<char> = "0123456789ABCDEF".chars().collect();
    (0..8).map(|_| chars[rng.gen_range(0..chars.len())]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_random_short_id() {
        let id = generate_random_short_id();
        assert_eq!(id.len(), 8);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit() && c.is_uppercase() || c.is_ascii_digit()));
    }

    #[test]
    fn test_derive_secret_key_valid() {
        assert!(derive_secret_key_from_short_id("A1B2C3D4").is_ok());
        assert!(derive_secret_key_from_short_id("a1b2c3d4").is_ok()); // Should normalize lowercase
    }

    #[test]
    fn test_derive_secret_key_invalid_length() {
        assert!(derive_secret_key_from_short_id("A1B2C3D").is_err());
        assert!(derive_secret_key_from_short_id("A1B2C3D45").is_err());
    }

    #[test]
    fn test_derive_secret_key_invalid_chars() {
        assert!(derive_secret_key_from_short_id("G1B2C3D4").is_err());
        assert!(derive_secret_key_from_short_id("A1-2C3D4").is_err());
    }

    #[test]
    fn test_derive_deterministic() {
        let key1 = derive_secret_key_from_short_id("A1B2C3D4").unwrap();
        let key2 = derive_secret_key_from_short_id("a1b2c3d4").unwrap();
        assert_eq!(key1.public(), key2.public());
    }
}
