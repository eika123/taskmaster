//! # Authentication & Account Lifecycle Test Campaign
//!
//! This test suite validates authentication against the real PostgreSQL database and Axum session layers:
//! - Testing a test database user with a known test session token.
//! - The full lifecycle flow: registration -> session verification -> logout -> session revocation -> login.
//! - Guardrails for registration (empty name, short password, duplicate email).
//! - Guardrails for login (non-existent email, wrong password).

use crate::backend::app_endpoints::{get_auth_user, get_current_user, login, logout, register};
use crate::backend::app_endpoints::tests::test_helpers::{seed_test_session, with_test_session};
use crate::backend::database::get_db;

/// Verifies that a test database populated with a test user and a known test session token
/// can be authenticated, and that missing or invalid tokens are rejected.
#[tokio::test]
async fn test_known_test_user_session_token() {
    let _pool = get_db().await;
    let known_token = "known_test_session_token_alice_12345";

    // Seed a known test session for Alice (user id: 1) in the test database
    seed_test_session(1, known_token).await;

    // 1. Authenticated request using the known test session token succeeds
    let auth_user = with_test_session(Some(known_token), || async {
        get_auth_user().await
    })
    .await
    .expect("Known session token must authenticate successfully");

    assert_eq!(auth_user.id, 1);
    assert_eq!(auth_user.name, "Alice Johnson");
    assert_eq!(auth_user.email, "alice@example.com");

    // 2. Unauthenticated request without a session token fails with Unauthorized
    let unauth_err = with_test_session(None, || async {
        get_auth_user().await
    })
    .await;
    assert!(unauth_err.is_err(), "Missing session must return Unauthorized");
    assert!(unauth_err.unwrap_err().to_string().contains("Unauthorized"));

    // 3. Request with an unknown / invalid session token fails with Unauthorized
    let invalid_err = with_test_session(Some("unknown_bogus_token_xyz"), || async {
        get_auth_user().await
    })
    .await;
    assert!(invalid_err.is_err(), "Invalid session token must return Unauthorized");
    assert!(invalid_err.unwrap_err().to_string().contains("Unauthorized"));
}

/// Verifies the full user authentication lifecycle:
/// 1. Register a new account (stores Argon2id hash and commits a real session).
/// 2. Verify authentication and session resolution with the returned session token.
/// 3. Logout (purges the session from the PostgreSQL database and revokes authentication).
/// 4. Verify post-logout access is rejected.
/// 5. Log back in with the credentials (verifies Argon2id hash and issues a new session).
/// 6. Verify authentication with the new session.
#[tokio::test]
async fn test_registration_logout_and_login_flow() {
    let pool = get_db().await;
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let email = format!("flowuser_{unique}@example.com");
    let password = "SuperSecretPassword123!";

    // --- STEP 1: Registration ---
    let account_email = email.clone();
    let registration_response = with_test_session(None, move || async move {
        register("Flow Tester".to_string(), account_email, password.to_string()).await
    })
    .await
    .expect("Registration must succeed");

    assert_eq!(registration_response.user.name, "Flow Tester");
    assert_eq!(registration_response.user.email, email);
    assert!(!registration_response.token.is_empty(), "Registration must return a session token");

    // Verify Argon2id password hash in PostgreSQL
    let stored_hash: String = sqlx::query_scalar("SELECT password_hash FROM \"user\" WHERE id = $1")
        .bind(registration_response.user.id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert!(stored_hash.starts_with("$argon2id$"), "Password must be hashed with Argon2id");

    // Verify session row exists in PostgreSQL `sessions` table
    let session_exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sessions WHERE id = $1)")
        .bind(&registration_response.token)
        .fetch_one(pool)
        .await
        .unwrap();
    assert!(session_exists, "Session record must exist in PostgreSQL sessions table");

    // --- STEP 2: Authenticated Access using Registered Session ---
    let current_user = with_test_session(Some(&registration_response.token), || async {
        get_auth_user().await
    })
    .await
    .expect("Registered session token must authenticate");
    assert_eq!(current_user.email, email);

    let resolved_user = get_current_user(Some(registration_response.token.clone())).await.unwrap();
    assert_eq!(resolved_user.map(|u| u.email), Some(email.clone()));

    // --- STEP 3: Logout ---
    let registration_token = registration_response.token.clone();
    let token_to_logout = registration_token.clone();
    with_test_session(Some(&registration_token), move || async move {
        logout(Some(token_to_logout)).await
    })
    .await
    .expect("Logout must succeed");

    // Verify session was deleted from PostgreSQL
    let session_still_exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sessions WHERE id = $1)")
        .bind(&registration_token)
        .fetch_one(pool)
        .await
        .unwrap();
    assert!(!session_still_exists, "Session must be removed from database after logout");

    // --- STEP 4: Verify Revocation ---
    let post_logout_auth = with_test_session(Some(&registration_token), || async {
        get_auth_user().await
    })
    .await;
    assert!(post_logout_auth.is_err(), "Logged out session must fail get_auth_user");
    assert!(post_logout_auth.unwrap_err().to_string().contains("Unauthorized"));

    let post_logout_current = get_current_user(Some(registration_token.clone())).await.unwrap();
    assert_eq!(post_logout_current, None, "Logged out session must not resolve current user");

    // --- STEP 5: Login ---
    // Incorrect password must fail
    let attempt_email = email.clone();
    let wrong_password_result = with_test_session(None, move || async move {
        login(attempt_email, "WrongPassword999!".to_string()).await
    })
    .await;
    assert!(wrong_password_result.is_err());
    assert!(wrong_password_result.unwrap_err().to_string().contains("Invalid email or password"));

    // Correct credentials succeed
    let login_email = email.clone();
    let login_response = with_test_session(None, move || async move {
        login(login_email, password.to_string()).await
    })
    .await
    .expect("Login with correct credentials must succeed");

    assert_eq!(login_response.user.email, email);
    assert!(!login_response.token.is_empty(), "Login must return a new session token");

    // --- STEP 6: Verify New Session ---
    let reauth_user = with_test_session(Some(&login_response.token), || async {
        get_auth_user().await
    })
    .await
    .expect("New session token from login must authenticate");
    assert_eq!(reauth_user.email, email);
}

/// Verifies registration input guardrails and collision detection.
#[tokio::test]
async fn test_registration_guardrails() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let email = format!("guardrail_{unique}@example.com");

    // 1. Empty name should be rejected
    let test_email = email.clone();
    let empty_name_result = with_test_session(None, move || async move {
        register("".to_string(), test_email, "password123".to_string()).await
    })
    .await;
    assert!(empty_name_result.is_err());
    assert!(empty_name_result.unwrap_err().to_string().contains("Name cannot be empty"));

    // 2. Short password (< 6 chars) should be rejected
    let test_email = email.clone();
    let short_password_result = with_test_session(None, move || async move {
        register("Valid Name".to_string(), test_email, "12345".to_string()).await
    })
    .await;
    assert!(short_password_result.is_err());
    assert!(short_password_result.unwrap_err().to_string().contains("Password must be at least 6 characters"));

    // 3. First valid registration succeeds
    let test_email = email.clone();
    let valid_registration_result = with_test_session(None, move || async move {
        register("Valid User".to_string(), test_email, "securepassword".to_string()).await
    })
    .await;
    assert!(valid_registration_result.is_ok());

    // 4. Duplicate email registration must be rejected
    let test_email = email.clone();
    let duplicate_email_result = with_test_session(None, move || async move {
        register("Another Name".to_string(), test_email, "anotherpass123".to_string()).await
    })
    .await;
    assert!(duplicate_email_result.is_err());
    assert!(duplicate_email_result.unwrap_err().to_string().contains("An account with this email already exists"));
}

/// Verifies login credential validation for non-existent users.
#[tokio::test]
async fn test_login_guardrails() {
    let non_existent = with_test_session(None, || async {
        login("nobody_exists_12345@example.com".to_string(), "anypassword".to_string()).await
    })
    .await;
    assert!(non_existent.is_err());
    assert!(non_existent.unwrap_err().to_string().contains("Invalid email or password"));
}
