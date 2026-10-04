use dioxus::prelude::*;
use crate::types::{AuthResponse, Task, TaskStatus, User};

#[cfg(feature = "server")]
use crate::backend::database::get_db;
#[cfg(feature = "server")]
use crate::backend::crypto::{hash_password, verify_password, RawUserWithPassword};
#[cfg(feature = "server")]
use crate::backend::middleware::rate_limiter::get_rate_limiter;
#[cfg(feature = "server")]
use crate::backend::middleware::auth::{AppAuthSession, SqlxSqliteSessionPool};

#[cfg(feature = "server")]
pub async fn get_auth_user() -> Result<User, ServerFnError> {
    let auth = dioxus_fullstack::FullstackContext::extract::<AppAuthSession, _>().await
        .map_err(|e| ServerFnError::new(format!("Failed to extract auth session: {e}")))?;
    auth.current_user.ok_or_else(|| ServerFnError::new("Unauthorized: Authentication required"))
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

    let auth = dioxus_fullstack::FullstackContext::extract::<AppAuthSession, _>().await
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
pub async fn register(name: String, email: String, password: String) -> Result<AuthResponse, ServerFnError> {
    let name_clean = name.trim().to_string();
    let email_clean = email.trim().to_lowercase();

    if name_clean.is_empty() {
        return Err(ServerFnError::new("Name cannot be empty."));
    }
    if email_clean.is_empty() || !email_clean.contains('@') {
        return Err(ServerFnError::new("Valid email is required."));
    }
    if password.len() < 6 {
        return Err(ServerFnError::new("Password must be at least 6 characters."));
    }

    let pool = get_db().await;
    let existing = sqlx::query_scalar::<_, i64>("SELECT id FROM user WHERE LOWER(email) = $1")
        .bind(&email_clean)
        .fetch_optional(pool)
        .await
        .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?;

    if existing.is_some() {
        return Err(ServerFnError::new("An account with this email already exists."));
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

    let auth = dioxus_fullstack::FullstackContext::extract::<AppAuthSession, _>().await
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
                    let user = sqlx::query_as::<_, User>("SELECT id, name, email FROM user WHERE id = $1")
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

#[cfg(feature = "server")]
#[derive(sqlx::FromRow)]
struct RawTask {
    id: i64,
    title: String,
    author: i64,
    author_name: String,
    description: Option<String>,
    status: TaskStatus,
}

#[server]
pub async fn suggest_task(
    task_title: String,
    description: String,
) -> Result<(), ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;
    sqlx::query(
        "INSERT INTO task (title, description, author, status) VALUES ($1, $2, $3, $4)",
    )
    .bind(task_title)
    .bind(description)
    .bind(user.id)
    .bind(TaskStatus::Suggested)
    .execute(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to create suggested task: {e}")))?;

    Ok(())
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

#[server]
pub async fn assign_task(task_id: i64, user_id: i64) -> Result<(), ServerFnError> {
    let pool = get_db().await;
    sqlx::query("INSERT OR IGNORE INTO assignments (task_id, assigned_to) VALUES ($1, $2)")
        .bind(task_id)
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to assign task: {e}")))?;

    Ok(())
}

#[server]
pub async fn unassign_task(task_id: i64, user_id: i64) -> Result<(), ServerFnError> {
    let pool = get_db().await;
    sqlx::query("DELETE FROM assignments WHERE task_id = $1 AND assigned_to = $2")
        .bind(task_id)
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to unassign task: {e}")))?;

    Ok(())
}

#[server]
pub async fn get_all_tasks(after_id: Option<i64>, amount: u64) -> Result<Vec<Task>, ServerFnError> {
    let last_id = after_id.unwrap_or(0);
    let pool = get_db().await;

    let raw_tasks = sqlx::query_as::<_, RawTask>(
        "SELECT task.id, task.title, task.author, user.name AS author_name, 
                task.description, task.status 
         FROM task 
         INNER JOIN user ON task.author = user.id 
         WHERE task.id > $1 
         ORDER BY task.id ASC 
         LIMIT $2",
    )
    .bind(last_id)
    .bind(amount as i64)
    .fetch_all(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to fetch tasks: {e}")))?;

    let mut tasks = Vec::with_capacity(raw_tasks.len());
    for raw in raw_tasks {
        let assignees = sqlx::query_as::<_, User>(
            "SELECT user.id, user.name, user.email 
             FROM assignments 
             INNER JOIN user ON assignments.assigned_to = user.id 
             WHERE assignments.task_id = $1 
             ORDER BY user.name ASC",
        )
        .bind(raw.id)
        .fetch_all(pool)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to fetch assignees: {e}")))?;

        tasks.push(Task {
            id: raw.id,
            title: raw.title,
            author: raw.author,
            author_name: raw.author_name,
            description: raw.description.unwrap_or_default(),
            status: raw.status,
            assignees,
        });
    }

    Ok(tasks)
}

#[server]
pub async fn get_task(task_id: i64) -> Result<Task, ServerFnError> {
    let pool = get_db().await;

    let raw = sqlx::query_as::<_, RawTask>(
        "SELECT task.id, task.title, task.author, user.name AS author_name, 
                task.description, task.status 
         FROM task 
         INNER JOIN user ON task.author = user.id 
         WHERE task.id = $1",
    )
    .bind(task_id)
    .fetch_one(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to fetch task: {e}")))?;

    let assignees = sqlx::query_as::<_, User>(
        "SELECT user.id, user.name, user.email 
         FROM assignments 
         INNER JOIN user ON assignments.assigned_to = user.id 
         WHERE assignments.task_id = $1 
         ORDER BY user.name ASC",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to fetch task assignees: {e}")))?;

    Ok(Task {
        id: raw.id,
        title: raw.title,
        author: raw.author,
        author_name: raw.author_name,
        description: raw.description.unwrap_or_default(),
        status: raw.status,
        assignees,
    })
}

#[server]
pub async fn delete_task(task_id: i64) -> Result<(), ServerFnError> {
    let pool = get_db().await;
    sqlx::query("DELETE FROM task WHERE id = $1")
        .bind(task_id)
        .execute(pool)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to delete task: {e}")))?;

    Ok(())
}

#[server]
pub async fn set_task_status(task_id: u64, status: TaskStatus) -> Result<(), ServerFnError> {
    let pool = get_db().await;
    sqlx::query("UPDATE task SET status = $1 WHERE id = $2")
        .bind(status)
        .bind(task_id as i64)
        .execute(pool)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to update task: {e}")))?;

    Ok(())
}