use crate::error::{ApiError, Result};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use sha2::{Digest, Sha256};

pub fn token() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}
pub fn digest(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}
pub fn hash_password(value: &str) -> Result<String> {
    appshell_domain::identity::password(value)?;
    Argon2::default()
        .hash_password(
            value.as_bytes(),
            &SaltString::generate(&mut rand_core::OsRng),
        )
        .map(|v| v.to_string())
        .map_err(ApiError::internal)
}
pub fn verify_password(value: &str, hash: &str) -> bool {
    value.len() <= 128
        && PasswordHash::new(hash).is_ok_and(|hash| {
            Argon2::default()
                .verify_password(value.as_bytes(), &hash)
                .is_ok()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn secrets_and_passwords() {
        let value = token();
        assert_eq!(value.len(), 64);
        assert_ne!(value, token());
        assert_ne!(digest(&value), value);
        let hash = hash_password("correct horse battery staple").unwrap();
        assert!(verify_password("correct horse battery staple", &hash));
        assert!(!verify_password("different", &hash));
    }
}
