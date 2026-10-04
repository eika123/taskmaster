#[cfg(feature = "server")]
use crate::types::User;

#[cfg(feature = "server")]
#[derive(Clone, Debug)]
pub struct SqlxSqliteSessionPool(pub sqlx::SqlitePool);

#[cfg(feature = "server")]
#[async_trait::async_trait]
impl axum_session::DatabasePool for SqlxSqliteSessionPool {
    async fn initiate(&self, table_name: &str) -> Result<(), axum_session::DatabaseError> {
        let query = format!(
            "CREATE TABLE IF NOT EXISTS {table_name} (
                id TEXT PRIMARY KEY NOT NULL,
                session TEXT NOT NULL,
                expires INTEGER NOT NULL
            );"
        );
        sqlx::query(&query)
            .execute(&self.0)
            .await
            .map_err(|e| axum_session::DatabaseError::GenericCreateError(e.to_string()))?;
        Ok(())
    }

    async fn count(&self, table_name: &str) -> Result<i64, axum_session::DatabaseError> {
        let query = format!("SELECT COUNT(*) FROM {table_name}");
        let count: i64 = sqlx::query_scalar(&query)
            .fetch_one(&self.0)
            .await
            .map_err(|e| ax_session_database_err(e))?;
        Ok(count)
    }

    async fn store(
        &self,
        id: &str,
        session: &str,
        expires: i64,
        table_name: &str,
    ) -> Result<(), axum_session::DatabaseError> {
        let query = format!(
            "INSERT INTO {table_name} (id, session, expires) VALUES ($1, $2, $3)
             ON CONFLICT(id) DO UPDATE SET session = excluded.session, expires = excluded.expires"
        );
        sqlx::query(&query)
            .bind(id)
            .bind(session)
            .bind(expires)
            .execute(&self.0)
            .await
            .map_err(|e| axum_session::DatabaseError::GenericInsertError(e.to_string()))?;
        Ok(())
    }

    async fn load(&self, id: &str, table_name: &str) -> Result<Option<String>, axum_session::DatabaseError> {
        let now = chrono::Utc::now().timestamp();
        let query = format!("SELECT session FROM {table_name} WHERE id = $1 AND expires > $2");
        let session = sqlx::query_scalar(&query)
            .bind(id)
            .bind(now)
            .fetch_optional(&self.0)
            .await
            .map_err(|e| ax_session_database_err(e))?;
        Ok(session)
    }

    async fn delete_one_by_id(&self, id: &str, table_name: &str) -> Result<(), axum_session::DatabaseError> {
        let query = format!("DELETE FROM {table_name} WHERE id = $1");
        sqlx::query(&query)
            .bind(id)
            .execute(&self.0)
            .await
            .map_err(|e| axum_session::DatabaseError::GenericDeleteError(e.to_string()))?;
        Ok(())
    }

    async fn exists(&self, id: &str, table_name: &str) -> Result<bool, axum_session::DatabaseError> {
        let now = chrono::Utc::now().timestamp();
        let query = format!("SELECT EXISTS(SELECT 1 FROM {table_name} WHERE id = $1 AND expires > $2)");
        let exists: bool = sqlx::query_scalar(&query)
            .bind(id)
            .bind(now)
            .fetch_one(&self.0)
            .await
            .map_err(|e| ax_session_database_err(e))?;
        Ok(exists)
    }

    async fn delete_by_expiry(&self, table_name: &str) -> Result<Vec<String>, axum_session::DatabaseError> {
        let now = chrono::Utc::now().timestamp();
        let select_query = format!("SELECT id FROM {table_name} WHERE expires <= $1");
        let ids: Vec<String> = sqlx::query_scalar(&select_query)
            .bind(now)
            .fetch_all(&self.0)
            .await
            .map_err(|e| ax_session_database_err(e))?;

        let delete_query = format!("DELETE FROM {table_name} WHERE expires <= $1");
        sqlx::query(&delete_query)
            .bind(now)
            .execute(&self.0)
            .await
            .map_err(|e| axum_session::DatabaseError::GenericDeleteError(e.to_string()))?;

        Ok(ids)
    }

    async fn delete_all(&self, table_name: &str) -> Result<(), axum_session::DatabaseError> {
        let query = format!("DELETE FROM {table_name}");
        sqlx::query(&query)
            .execute(&self.0)
            .await
            .map_err(|e| axum_session::DatabaseError::GenericDeleteError(e.to_string()))?;
        Ok(())
    }

    async fn get_ids(&self, table_name: &str) -> Result<Vec<String>, axum_session::DatabaseError> {
        let now = chrono::Utc::now().timestamp();
        let query = format!("SELECT id FROM {table_name} WHERE expires > $1");
        let ids: Vec<String> = sqlx::query_scalar(&query)
            .bind(now)
            .fetch_all(&self.0)
            .await
            .map_err(|e| ax_session_database_err(e))?;
        Ok(ids)
    }

    fn auto_handles_expiry(&self) -> bool {
        false
    }
}

#[cfg(feature = "server")]
fn ax_session_database_err(err: sqlx::Error) -> axum_session::DatabaseError {
    axum_session::DatabaseError::GenericSelectError(err.to_string())
}

#[cfg(feature = "server")]
pub type AppAuthSession = axum_session_auth::AuthSession<User, i64, SqlxSqliteSessionPool, sqlx::SqlitePool>;

#[cfg(feature = "server")]
pub async fn create_session_store(
    pool: sqlx::SqlitePool,
) -> Result<axum_session::SessionStore<SqlxSqliteSessionPool>, anyhow::Error> {
    let session_config = axum_session::SessionConfig::default()
        .with_table_name("sessions")
        .with_session_name("flowtask_session")
        .with_prefix_with_host(false)
        .with_secure(false)
        .with_mode(axum_session::SessionMode::Persistent);

    let session_store = axum_session::SessionStore::<SqlxSqliteSessionPool>::new(
        Some(SqlxSqliteSessionPool(pool)),
        session_config,
    )
    .await?;
    Ok(session_store)
}

#[cfg(feature = "server")]
pub fn create_auth_layer(
    pool: sqlx::SqlitePool,
) -> axum_session_auth::AuthSessionLayer<User, i64, SqlxSqliteSessionPool, sqlx::SqlitePool> {
    let auth_config = axum_session_auth::AuthConfig::<i64>::default();
    axum_session_auth::AuthSessionLayer::<User, i64, SqlxSqliteSessionPool, sqlx::SqlitePool>::new(Some(pool))
        .with_config(auth_config)
}

#[cfg(feature = "server")]
pub fn is_public_path(path: &str) -> bool {
    path == "/"
        || path == "/login"
        || path == "/register"
        || path == "/favicon.ico"
        || path.starts_with("/assets")
        || path.starts_with("/wasm")
        || path.starts_with("/_dioxus")
        || path.starts_with("/api/login")
        || path.starts_with("/api/register")
        || path.starts_with("/api/get_current_user")
        || path.starts_with("/api/logout")
        || path.ends_with(".wasm")
        || path.ends_with(".js")
        || path.ends_with(".css")
        || path.ends_with(".ico")
        || path.ends_with(".png")
        || path.ends_with(".jpg")
        || path.ends_with(".svg")
        || path.ends_with(".map")
}

#[cfg(feature = "server")]
pub async fn auth_guard_middleware(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;

    let path = request.uri().path();

    if is_public_path(path) {
        return next.run(request).await;
    }

    let is_authenticated = request
        .extensions()
        .get::<AppAuthSession>()
        .map(|auth| auth.is_authenticated())
        .unwrap_or(false);

    if is_authenticated {
        next.run(request).await
    } else if path.starts_with("/api/") {
        (
            http::StatusCode::UNAUTHORIZED,
            [("content-type", "application/json")],
            r#"{"error":"Unauthorized: Authentication required"}"#,
        )
            .into_response()
    } else {
        axum::response::Redirect::to("/login").into_response()
    }
}

#[cfg(feature = "server")]
pub async fn auth_header_to_cookie_middleware(
    mut request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let has_flowtask_cookie = request
        .headers()
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.contains("flowtask_session="))
        .unwrap_or(false);

    if !has_flowtask_cookie {
        if let Some(auth_val) = request.headers().get("authorization").and_then(|v| v.to_str().ok()) {
            if let Some(token) = auth_val.strip_prefix("Bearer ") {
                let token = token.trim();
                if !token.is_empty() {
                    let new_cookie = if let Some(existing) = request.headers().get("cookie").and_then(|v| v.to_str().ok()) {
                        format!("{}; flowtask_session={}", existing, token)
                    } else {
                        format!("flowtask_session={}", token)
                    };
                    if let Ok(cookie_val) = axum::http::HeaderValue::from_str(&new_cookie) {
                        request.headers_mut().insert("cookie", cookie_val);
                    }
                }
            }
        }
    }
    next.run(request).await
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sqlite_session_pool() {
        use axum_session::DatabasePool;
        let pool = crate::backend::database::get_db().await;
        let session_pool = SqlxSqliteSessionPool(pool.clone());
        session_pool.initiate("sessions").await.unwrap();

        let session_id = "test_session_id_123";
        let session_data = r#"{"user_auth_session_id":1,"data":"xyz"}"#;
        let expires = chrono::Utc::now().timestamp() + 3600;

        session_pool.store(session_id, session_data, expires, "sessions").await.unwrap();
        assert!(session_pool.exists(session_id, "sessions").await.unwrap());

        let loaded = session_pool.load(session_id, "sessions").await.unwrap();
        assert_eq!(loaded.as_deref(), Some(session_data));

        session_pool.delete_one_by_id(session_id, "sessions").await.unwrap();
        assert!(!session_pool.exists(session_id, "sessions").await.unwrap());
    }

    #[test]
    fn test_public_path_rules() {
        // Public paths
        assert!(is_public_path("/"));
        assert!(is_public_path("/login"));
        assert!(is_public_path("/register"));
        assert!(is_public_path("/assets/main.css"));
        assert!(is_public_path("/wasm/todolist.js"));
        assert!(is_public_path("/wasm/todolist_bg.wasm"));
        assert!(is_public_path("/favicon.ico"));
        assert!(is_public_path("/_dioxus/ws"));
        assert!(is_public_path("/api/login12345"));
        assert!(is_public_path("/api/register12345"));
        assert!(is_public_path("/api/get_current_user12345"));
        assert!(is_public_path("/api/logout12345"));

        // Protected paths
        assert!(!is_public_path("/tasks"));
        assert!(!is_public_path("/task/1"));
        assert!(!is_public_path("/task/new"));
        assert!(!is_public_path("/api/get_all_tasks12345"));
        assert!(!is_public_path("/api/suggest_task12345"));
        assert!(!is_public_path("/api/get_users12345"));
        assert!(!is_public_path("/api/delete_task12345"));
        assert!(!is_public_path("/api/set_task_status12345"));
    }

    #[tokio::test]
    async fn test_auth_guard_middleware_blocks_unauthenticated() {
        use axum::{body::Body, routing::post, Router};
        use tower::ServiceExt;

        let app = Router::new()
            .route("/api/get_all_tasks123", post(|| async { "tasks data" }))
            .route("/tasks", axum::routing::get(|| async { "tasks page" }))
            .layer(axum::middleware::from_fn(auth_guard_middleware));

        // 1. Unauthenticated request to protected API must return 401
        let req = axum::extract::Request::builder()
            .uri("/api/get_all_tasks123")
            .method("POST")
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), http::StatusCode::UNAUTHORIZED);

        // 2. Unauthenticated request to protected HTML page must redirect to /login
        let req = axum::extract::Request::builder()
            .uri("/tasks")
            .method("GET")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), http::StatusCode::SEE_OTHER);
        assert_eq!(resp.headers().get("location").unwrap(), "/login");
    }

    #[tokio::test]
    async fn test_render_login_page() {
        use axum::body::{to_bytes, Body};
        use tower::ServiceExt;

        let public_dir = std::path::Path::new("target/dx/todolist/debug/web/public");
        if public_dir.exists() {
            std::env::set_var("DIOXUS_PUBLIC_PATH", public_dir);
            let router = dioxus::server::router(crate::App);
            let req = axum::extract::Request::builder()
                .uri("/login")
                .method("GET")
                .body(Body::empty())
                .unwrap();
            let resp = router.oneshot(req).await.unwrap();
            assert_eq!(resp.status(), http::StatusCode::OK);
            let bytes = to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
            let html = String::from_utf8_lossy(&bytes);
            assert!(html.contains("FlowTask") || html.contains("login") || html.contains("main"));
        }
    }
}

#[cfg(feature = "server")]
#[async_trait::async_trait]
impl axum_session_auth::Authentication<User, i64, sqlx::SqlitePool> for User {
    async fn load_user(userid: i64, pool: Option<&sqlx::SqlitePool>) -> Result<User, anyhow::Error> {
        let pool = match pool {
            Some(p) => p,
            None => crate::backend::database::get_db().await,
        };
        let user = sqlx::query_as::<_, User>("SELECT id, name, email FROM user WHERE id = $1")
            .bind(userid)
            .fetch_one(pool)
            .await
            .map_err(|e| anyhow::anyhow!("User {userid} not found: {e}"))?;
        Ok(user)
    }

    fn is_authenticated(&self) -> bool {
        self.id > 0
    }

    fn is_active(&self) -> bool {
        self.id > 0
    }

    fn is_anonymous(&self) -> bool {
        self.id <= 0
    }
}