use anyhow::Result;
use rand_core::OsRng;
use x25519_dalek::{EphemeralSecret, PublicKey, SharedSecret};

pub struct EcdhKeyExchange {
    secret: EphemeralSecret,
    public: PublicKey,
}

impl EcdhKeyExchange {
    pub fn new() -> Self {
        let secret = EphemeralSecret::random_from_rng(OsRng);
        let public = PublicKey::from(&secret);
        Self { secret, public }
    }

    pub fn public_key_bytes(&self) -> [u8; 32] {
        *self.public.as_bytes()
    }

    pub fn derive_shared_secret(self, peer_public_bytes: &[u8; 32]) -> Result<[u8; 32]> {
        let peer_public = PublicKey::from(*peer_public_bytes);
        let shared: SharedSecret = self.secret.diffie_hellman(&peer_public);
        Ok(*shared.as_bytes())
    }
}

impl Default for EcdhKeyExchange {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ecdh_shared_secret_matches() {
        let alice = EcdhKeyExchange::new();
        let bob = EcdhKeyExchange::new();

        let alice_pub = alice.public_key_bytes();
        let bob_pub = bob.public_key_bytes();

        let alice_shared = alice.derive_shared_secret(&bob_pub).unwrap();
        let bob_shared = bob.derive_shared_secret(&alice_pub).unwrap();

        assert_eq!(alice_shared, bob_shared, "Shared secrets must match");
    }

    #[test]
    fn test_ecdh_different_peers_different_secrets() {
        let bob = EcdhKeyExchange::new();
        let carol = EcdhKeyExchange::new();

        let bob_pub = bob.public_key_bytes();
        let carol_pub = carol.public_key_bytes();

        let alice_bob = EcdhKeyExchange::new()
            .derive_shared_secret(&bob_pub)
            .unwrap();
        let alice_carol = EcdhKeyExchange::new()
            .derive_shared_secret(&carol_pub)
            .unwrap();

        assert_ne!(
            alice_bob, alice_carol,
            "Different peers must produce different shared secrets"
        );
    }

    #[test]
    fn test_public_key_is_32_bytes() {
        let kx = EcdhKeyExchange::new();
        assert_eq!(kx.public_key_bytes().len(), 32);
    }
}
