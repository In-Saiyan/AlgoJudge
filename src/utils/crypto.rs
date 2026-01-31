//! Cryptographic utilities

use rand::Rng;
use sha2::{Digest, Sha256};

/// Generate a cryptographically secure random token
pub fn generate_secure_token(length: usize) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::rng();

    (0..length)
        .map(|_| {
            let idx = rng.random_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

/// Hash a string using SHA-256
pub fn hash_string(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)
}

/// Generate a random invite code
pub fn generate_invite_code() -> String {
    generate_secure_token(8).to_uppercase()
}

/// Verify a hash matches the input
pub fn verify_hash(input: &str, hash: &str) -> bool {
    hash_string(input) == hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_secure_token() {
        let token1 = generate_secure_token(32);
        let token2 = generate_secure_token(32);

        assert_eq!(token1.len(), 32);
        assert_eq!(token2.len(), 32);
        assert_ne!(token1, token2);
    }

    #[test]
    fn test_hash_string() {
        let hash1 = hash_string("test");
        let hash2 = hash_string("test");
        let hash3 = hash_string("different");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_verify_hash() {
        let input = "test_input";
        let hash = hash_string(input);

        assert!(verify_hash(input, &hash));
        assert!(!verify_hash("wrong_input", &hash));
    }
}
