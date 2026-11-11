use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use mc_core::{CoreError, Result};
use rand::RngCore;

const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 24;

#[derive(Clone)]
pub struct MasterKey([u8; KEY_LEN]);

impl MasterKey {
    pub fn from_encoded(encoded: &str) -> Result<Self> {
        let encoded = encoded.trim();
        let bytes = hex::decode(encoded)
            .ok()
            .filter(|bytes| bytes.len() == KEY_LEN)
            .or_else(|| {
                STANDARD
                    .decode(encoded)
                    .ok()
                    .filter(|bytes| bytes.len() == KEY_LEN)
            })
            .ok_or_else(|| {
                CoreError::Validation(
                    "master key must be 32 bytes encoded as hex or base64".to_owned(),
                )
            })?;

        let mut key = [0_u8; KEY_LEN];
        key.copy_from_slice(&bytes);
        Ok(Self(key))
    }

    #[must_use]
    pub fn generate() -> Self {
        let mut key = [0_u8; KEY_LEN];
        rand::rngs::OsRng.fill_bytes(&mut key);
        Self(key)
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> Result<String> {
        let cipher = XChaCha20Poly1305::new_from_slice(&self.0)
            .map_err(|error| CoreError::Internal(format!("cipher init: {error}")))?;
        let mut nonce_bytes = [0_u8; NONCE_LEN];
        rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = XNonce::try_from(&nonce_bytes[..])
            .map_err(|_| CoreError::Internal("invalid nonce length".to_owned()))?;

        let ciphertext = cipher
            .encrypt(&nonce, plaintext)
            .map_err(|error| CoreError::Internal(format!("encrypt secret: {error}")))?;

        let mut payload = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        payload.extend_from_slice(&nonce_bytes);
        payload.extend_from_slice(&ciphertext);

        Ok(STANDARD.encode(payload))
    }

    pub fn decrypt(&self, encoded: &str) -> Result<Vec<u8>> {
        let payload = STANDARD
            .decode(encoded.trim())
            .map_err(|error| CoreError::Validation(format!("decode secret: {error}")))?;

        if payload.len() <= NONCE_LEN {
            return Err(CoreError::Validation(
                "encrypted secret is truncated".to_owned(),
            ));
        }

        let (nonce_bytes, ciphertext) = payload.split_at(NONCE_LEN);
        let cipher = XChaCha20Poly1305::new_from_slice(&self.0)
            .map_err(|error| CoreError::Internal(format!("cipher init: {error}")))?;
        let nonce = XNonce::try_from(nonce_bytes)
            .map_err(|_| CoreError::Internal("invalid nonce length".to_owned()))?;

        cipher
            .decrypt(&nonce, ciphertext)
            .map_err(|error| CoreError::Validation(format!("decrypt secret: {error}")))
    }
}

impl std::fmt::Debug for MasterKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MasterKey([redacted])")
    }
}

#[cfg(test)]
mod tests {
    use base64::Engine;

    use super::MasterKey;

    #[test]
    fn encrypts_and_decrypts_secrets() {
        let key = MasterKey::generate();
        let encrypted = key.encrypt(b"s3-secret-key").unwrap();

        assert_ne!(encrypted, "s3-secret-key");
        assert_eq!(key.decrypt(&encrypted).unwrap(), b"s3-secret-key");
    }

    #[test]
    fn rejects_tampered_ciphertext() {
        let key = MasterKey::generate();
        let mut encrypted = key.encrypt(b"secret").unwrap();
        encrypted.push('A');

        assert!(key.decrypt(&encrypted).is_err());
    }

    #[test]
    fn parses_hex_and_base64_keys() {
        let key = MasterKey::generate();
        let hex = hex::encode(key.0);
        let base64 = base64::engine::general_purpose::STANDARD.encode(key.0);

        assert!(MasterKey::from_encoded(&hex).is_ok());
        assert!(MasterKey::from_encoded(&base64).is_ok());
        assert!(MasterKey::from_encoded("short").is_err());
    }
}
