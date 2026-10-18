#[cfg(feature = "server")]
#[derive(sqlx::FromRow)]
pub struct RawUserWithPassword {
    pub id: i64,
    pub name: String,
    pub email: String,
    pub password_hash: String,
}

const DEFAULT_PEPPER: &[u8] = b"flowtask_secret_pepper_seed_2026";

#[cfg(feature = "server")]
pub fn get_pepper() -> Vec<u8> {
    std::env::var("TODOLIST_PEPPER")
        .map(|s| s.into_bytes())
        .unwrap_or_else(|_| DEFAULT_PEPPER.to_vec())
}

#[cfg(feature = "server")]
pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    use argon2::{
        password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
        Algorithm, Argon2, Params, Version,
    };
    let pepper = get_pepper();
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::new_with_secret(
        &pepper,
        Algorithm::Argon2id,
        Version::V0x13,
        Params::default(),
    )?;
    let hash = argon2.hash_password(password.as_bytes(), &salt)?;
    Ok(hash.to_string())
}

#[cfg(feature = "server")]
pub fn verify_password(password: &str, hash: &str) -> Result<bool, argon2::password_hash::Error> {
    use argon2::{
        password_hash::{PasswordHash, PasswordVerifier},
        Algorithm, Argon2, Params, Version,
    };
    let pepper = get_pepper();
    let parsed_hash = PasswordHash::new(hash)?;
    let argon2 = Argon2::new_with_secret(
        &pepper,
        Algorithm::Argon2id,
        Version::V0x13,
        Params::default(),
    )?;
    Ok(argon2
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

#[cfg(feature = "server")]
#[allow(dead_code)]
pub fn generate_token() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;

    /// Verifies Argon2id password hashing and password verification with server pepper:
    /// - Output uses the standardized Argon2id PHC format `$argon2id$v=19$...`.
    /// - Exact password passes verification.
    /// - Incorrect password fails verification.
    #[test]
    fn test_password_hashing_and_verification() {
        let password = "SuperSecretPassword123!";
        let hash = hash_password(password).expect("Hashing should succeed");
        assert!(hash.starts_with("$argon2id$"));

        let is_valid = verify_password(password, &hash).expect("Verification should succeed");
        assert!(is_valid, "Correct password must verify");

        let is_invalid =
            verify_password("WrongPassword!", &hash).expect("Verification should succeed");
        assert!(!is_invalid, "Wrong password must fail verification");
    }

    /// Verifies backwards-compatibility against seeded database password hashes.
    #[test]
    fn test_verify_existing_db_hash() {
        let seeded_hash = "$argon2id$v=19$m=19456,t=2,p=1$7Qk0X0PIR5SuTQmBcdeqfQ$NOqm0J876/r/RCYG3rMGQmOZj27hgKE3DwzKjiJ5tYs";
        let is_valid =
            verify_password("password123", seeded_hash).expect("Verification should succeed");
        assert!(is_valid, "Seeded user passwords must verify with Argon2id");
    }
}
