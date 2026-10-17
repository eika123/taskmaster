use dioxus::prelude::*;
use crate::types::{AuthResponse, User};

#[cfg(feature = "server")]
use crate::backend::crypto::{hash_password, verify_password, RawUserWithPassword};
#[cfg(feature = "server")]
use crate::backend::database::get_db;
#[cfg(feature = "server")]
use crate::backend::middleware::auth::{AppAuthSession, SqlxSqliteSessionPool};
#[cfg(feature = "server")]
use crate::backend::middleware::rate_limiter::get_rate_limiter;

#[cfg(feature = "server")]
pub async fn get_auth_user() -> Result<User, ServerFnError> {
    let auth = dioxus_fullstack::FullstackContext::extract::<AppAuthSession, _>()
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to extract auth session: {e}")))?;
    auth.current_user
        .ok_or_else(|| ServerFnError::new("Unauthorized: Authentication required"))
}

#[server]
pub async fn login(email: String, password: String) -> Result<AuthResponse, ServerFnError> {
    let email_clean = email.trim().to_lowercase();
    let limiter = get_rate_limiter().await;

    if let Err(remaining) = limiter.check_limit(&email_clean).await {
        return Err(ServerFnError::new(format!(
            "Too many failed login attempts. Please wait {} seconds before trying again.",
            remaining.as_secs().max(1)
        )));
    }

    let pool = get_db().await;
    let user_row = sqlx::query_as::<_, RawUserWithPassword>(
        "SELECT id, name, email, password_hash FROM user WHERE LOWER(email) = $1",
    )
    .bind(&email_clean)
    .fetch_optional(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?;

    let user_row = match user_row {
        Some(u) => u,
        None => {
            limiter.record_failure(&email_clean).await;
            return Err(ServerFnError::new("Invalid email or password."));
        }
    };

    let pass_clone = password.clone();
    let hash_clone = user_row.password_hash.clone();
    let is_valid = tokio::task::spawn_blocking(move || {
        verify_password(&pass_clone, &hash_clone).unwrap_or(false)
    })
    .await
    .map_err(|e| ServerFnError::new(format!("Crypto task failed: {e}")))?;

    if !is_valid {
        limiter.record_failure(&email_clean).await;
        return Err(ServerFnError::new("Invalid email or password."));
    }

    limiter.record_success(&email_clean).await;

    let auth = dioxus_fullstack::FullstackContext::extract::<AppAuthSession, _>()
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to extract auth session: {e}")))?;

    auth.login_user(user_row.id);
    auth.remember_user(true);

    let session_token = auth.session.get_session_id();

    Ok(AuthResponse {
        token: session_token,
        user: User {
            id: user_row.id,
            name: user_row.name,
            email: user_row.email,
        },
    })
}

#[server]
pub async fn register(
    name: String,
    email: String,
    password: String,
) -> Result<AuthResponse, ServerFnError> {
    let name_clean = name.trim().to_string();
    let email_clean = email.trim().to_lowercase();

    if name_clean.is_empty() {
        return Err(ServerFnError::new("Name cannot be empty."));
    }
    if email_clean.is_empty() || !email_clean.contains('@') {
        return Err(ServerFnError::new("Valid email is required."));
    }
    if password.len() < 6 {
        return Err(ServerFnError::new(
            "Password must be at least 6 characters.",
        ));
    }

    let pool = get_db().await;
    let existing = sqlx::query_scalar::<_, i64>("SELECT id FROM user WHERE LOWER(email) = $1")
        .bind(&email_clean)
        .fetch_optional(pool)
        .await
        .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?;

    if existing.is_some() {
        return Err(ServerFnError::new(
            "An account with this email already exists.",
        ));
    }

    let pass_clone = password.clone();
    let password_hash = tokio::task::spawn_blocking(move || {
        hash_password(&pass_clone).map_err(|e| ServerFnError::new(format!("Hashing failed: {e}")))
    })
    .await
    .map_err(|e| ServerFnError::new(format!("Crypto task failed: {e}")))??;

    let res = sqlx::query("INSERT INTO user (name, email, password_hash) VALUES ($1, $2, $3)")
        .bind(&name_clean)
        .bind(&email_clean)
        .bind(&password_hash)
        .execute(pool)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to create user: {e}")))?;

    let user_id = res.last_insert_rowid();

    let auth = dioxus_fullstack::FullstackContext::extract::<AppAuthSession, _>()
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to extract auth session: {e}")))?;

    auth.login_user(user_id);
    auth.remember_user(true);

    let session_token = auth.session.get_session_id();

    Ok(AuthResponse {
        token: session_token,
        user: User {
            id: user_id,
            name: name_clean,
            email: email_clean,
        },
    })
}

#[server]
pub async fn logout(token: Option<String>) -> Result<(), ServerFnError> {
    if let Ok(auth) = dioxus_fullstack::FullstackContext::extract::<AppAuthSession, _>().await {
        auth.logout_user();
    }
    if let Some(t) = token.filter(|s| !s.is_empty()) {
        let pool = get_db().await;
        let session_pool = SqlxSqliteSessionPool(pool.clone());
        use axum_session::DatabasePool;
        let _ = session_pool.delete_one_by_id(&t, "sessions").await;
    }
    Ok(())
}

#[server]
pub async fn get_current_user(token: Option<String>) -> Result<Option<User>, ServerFnError> {
    if let Ok(auth) = dioxus_fullstack::FullstackContext::extract::<AppAuthSession, _>().await {
        if auth.is_authenticated() {
            return Ok(auth.current_user);
        }
    }

    if let Some(t) = token.filter(|s| !s.is_empty()) {
        let pool = get_db().await;
        let session_pool = SqlxSqliteSessionPool(pool.clone());
        use axum_session::DatabasePool;
        if let Ok(Some(session_json)) = session_pool.load(&t, "sessions").await {
            if let Ok(data) = serde_json::from_str::<serde_json::Value>(&session_json) {
                if let Some(user_id) = data.get("user_auth_session_id").and_then(|v| v.as_i64()) {
                    let user = sqlx::query_as::<_, User>(
                        "SELECT id, name, email FROM user WHERE id = $1",
                    )
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await
                    .ok()
                    .flatten();
                    return Ok(user);
                }
            }
        }
    }

    Ok(None)
}

#[server]
pub async fn get_users() -> Result<Vec<User>, ServerFnError> {
    let pool = get_db().await;
    let users = sqlx::query_as::<_, User>("SELECT id, name, email FROM user ORDER BY name ASC")
        .fetch_all(pool)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to fetch users: {e}")))?;

    Ok(users)
}

