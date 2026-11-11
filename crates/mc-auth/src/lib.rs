use argon2::password_hash::{
    rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use argon2::Argon2;
use mc_core::{CoreError, Result};
use md5::{Digest, Md5};
use rand::RngCore;

pub const SESSION_COOKIE: &str = "mc_session";

pub fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);

    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|error| CoreError::Internal(format!("password hash: {error}")))
}

pub fn verify_password(hash: &str, password: &str) -> Result<bool> {
    let parsed = PasswordHash::new(hash)
        .map_err(|error| CoreError::Internal(format!("password hash: {error}")))?;

    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

#[must_use]
pub fn generate_token() -> String {
    let mut bytes = [0_u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

#[must_use]
pub fn generate_secret() -> String {
    let mut bytes = [0_u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

#[must_use]
pub fn hash_token(token: &str) -> String {
    blake3::hash(token.as_bytes()).to_hex().to_string()
}

#[must_use]
pub fn subsonic_token(secret: &str, salt: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(secret.as_bytes());
    hasher.update(salt.as_bytes());
    hex::encode(hasher.finalize())
}

#[must_use]
pub fn verify_subsonic_token(secret: &str, salt: &str, token: &str) -> bool {
    subsonic_token(secret, salt).eq_ignore_ascii_case(token)
}

#[cfg(test)]
mod tests {
    use super::{
        generate_token, hash_password, hash_token, subsonic_token, verify_password,
        verify_subsonic_token,
    };

    #[test]
    fn passwords_roundtrip() {
        let hash = hash_password("correct horse battery staple").unwrap();
        assert!(verify_password(&hash, "correct horse battery staple").unwrap());
        assert!(!verify_password(&hash, "wrong").unwrap());
    }

    #[test]
    fn tokens_are_unique_and_hashed() {
        let first = generate_token();
        let second = generate_token();

        assert_eq!(first.len(), 64);
        assert_ne!(first, second);
        assert_eq!(hash_token(&first).len(), 64);
        assert_ne!(hash_token(&first), first);
    }

    #[test]
    fn subsonic_tokens_match_standard_md5() {
        assert_eq!(
            subsonic_token("secret", "salt"),
            "99cd2e5a95d555ee7be3b038a4a84625"
        );
        assert!(verify_subsonic_token(
            "secret",
            "salt",
            &subsonic_token("secret", "salt")
        ));
        assert!(!verify_subsonic_token("secret", "salt", "nope"));
    }
}
