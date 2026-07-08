use anyhow::Result;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use hkdf::Hkdf;
use rand_core::RngCore;
use sha2::Sha256;

pub const NONCE_LEN: usize = 12;
pub const KEY_LEN: usize = 32;
pub const TAG_LEN: usize = 16;

pub fn derive_encryption_key(
    shared_secret: &[u8; 32],
    salt: &[u8],
    info: &[u8],
) -> Result<[u8; KEY_LEN]> {
    let hk = Hkdf::<Sha256>::new(Some(salt), shared_secret);
    let mut okm = [0u8; KEY_LEN];
    hk.expand(info, &mut okm)
        .map_err(|e| anyhow::anyhow!("HKDF expand failed: {e:?}"))?;
    Ok(okm)
}

pub fn generate_nonce_prefix() -> [u8; 4] {
    let mut prefix = [0u8; 4];
    for b in prefix.iter_mut() {
        *b = rand_core::OsRng.next_u32() as u8;
    }
    prefix
}

pub fn build_nonce(prefix: &[u8; 4], counter: u64) -> [u8; NONCE_LEN] {
    let mut nonce = [0u8; NONCE_LEN];
    nonce[..4].copy_from_slice(prefix);
    nonce[4..].copy_from_slice(&counter.to_be_bytes());
    nonce
}

pub fn encrypt(
    key_bytes: &[u8; KEY_LEN],
    nonce: &[u8; NONCE_LEN],
    plaintext: &[u8],
) -> Result<Vec<u8>> {
    let key = Key::from_slice(key_bytes);
    let cipher = ChaCha20Poly1305::new(key);
    let nonce = Nonce::from_slice(nonce);
    cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| anyhow::anyhow!("Encryption failed: {e:?}"))
}

pub fn decrypt(
    key_bytes: &[u8; KEY_LEN],
    nonce: &[u8; NONCE_LEN],
    ciphertext: &[u8],
) -> Result<Vec<u8>> {
    let key = Key::from_slice(key_bytes);
    let cipher = ChaCha20Poly1305::new(key);
    let nonce = Nonce::from_slice(nonce);
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| anyhow::anyhow!("Decryption failed: {e:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> [u8; KEY_LEN] {
        [0xAB; KEY_LEN]
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = test_key();
        let prefix = [0x01, 0x02, 0x03, 0x04];
        let nonce = build_nonce(&prefix, 0);
        let plaintext = b"Hello, P2P world! This is a test message.";

        let ciphertext = encrypt(&key, &nonce, plaintext).unwrap();
        assert!(!ciphertext.is_empty());
        assert_eq!(ciphertext.len(), plaintext.len() + TAG_LEN);

        let decrypted = decrypt(&key, &nonce, &ciphertext).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_different_nonce_produces_different_ciphertext() {
        let key = test_key();
        let prefix = [0x01, 0x02, 0x03, 0x04];
        let plaintext = b"Same message";

        let ct1 = encrypt(&key, &build_nonce(&prefix, 0), plaintext).unwrap();
        let ct2 = encrypt(&key, &build_nonce(&prefix, 1), plaintext).unwrap();

        assert_ne!(
            ct1, ct2,
            "Different nonces must produce different ciphertexts"
        );
    }

    #[test]
    fn test_decrypt_wrong_key_fails() {
        let key1 = [0xAB; KEY_LEN];
        let key2 = [0xCD; KEY_LEN];
        let prefix = [0x01, 0x02, 0x03, 0x04];
        let nonce = build_nonce(&prefix, 0);
        let plaintext = b"Secret data";

        let ciphertext = encrypt(&key1, &nonce, plaintext).unwrap();
        let result = decrypt(&key2, &nonce, &ciphertext);

        assert!(result.is_err(), "Decryption with wrong key must fail");
    }

    #[test]
    fn test_decrypt_tampered_ciphertext_fails() {
        let key = test_key();
        let nonce = build_nonce(&[0x01, 0x02, 0x03, 0x04], 0);
        let plaintext = b"Tamper test";

        let mut ciphertext = encrypt(&key, &nonce, plaintext).unwrap();
        ciphertext[0] ^= 0xFF;

        let result = decrypt(&key, &nonce, &ciphertext);
        assert!(result.is_err(), "Tampered ciphertext must fail decryption");
    }

    #[test]
    fn test_derive_key_produces_consistent_keys() {
        let shared_secret = [0x42; 32];
        let salt = b"BLIP_SALT_v1";
        let info = b"BLIP encryption key";

        let key1 = derive_encryption_key(&shared_secret, salt, info).unwrap();
        let key2 = derive_encryption_key(&shared_secret, salt, info).unwrap();

        assert_eq!(key1, key2, "Same inputs must produce same key");
    }

    #[test]
    fn test_derive_key_different_info_produces_different_keys() {
        let shared_secret = [0x42; 32];
        let salt = b"BLIP_SALT_v1";

        let key1 = derive_encryption_key(&shared_secret, salt, b"info1").unwrap();
        let key2 = derive_encryption_key(&shared_secret, salt, b"info2").unwrap();

        assert_ne!(key1, key2, "Different info must produce different keys");
    }

    #[test]
    fn test_build_nonce_format() {
        let prefix = [0xDE, 0xAD, 0xBE, 0xEF];
        let nonce = build_nonce(&prefix, 0x0102030405060708);
        assert_eq!(nonce[..4], prefix);
        assert_eq!(nonce[4..], [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
    }

    #[test]
    fn test_generate_nonce_prefix_unique() {
        let p1 = generate_nonce_prefix();
        let p2 = generate_nonce_prefix();
        assert_ne!(p1, p2, "Generated prefixes should be unique");
    }
}
