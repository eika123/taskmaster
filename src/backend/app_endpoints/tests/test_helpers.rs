use crate::backend::database::get_db;
use crate::backend::middleware::auth::{
    auth_header_to_cookie_middleware, create_auth_layer, create_session_store,
    SqlxPostgresSessionPool,
};
use dioxus_fullstack::FullstackContext;
use http::Request;
use tower::{ServiceBuilder, ServiceExt};

/// Seeds a test session into the PostgreSQL `sessions` table for a known user and token.
pub async fn seed_test_session(user_id: i64, token: &str) {
    let pool = get_db().await;
    let session_pool = SqlxPostgresSessionPool(pool.clone());
    use axum_session::DatabasePool;
    let _ = session_pool.initiate("sessions").await;
    let mut data = std::collections::HashMap::new();
    data.insert("user_auth_session_id".to_string(), user_id.to_string());
    let session_data = serde_json::json!({
        "data": data,
        "longterm": true
    })
    .to_string();
    let expires = chrono::Utc::now().timestamp() + 86400 * 30;
    session_pool
        .store(token, &session_data, expires, "sessions")
        .await
        .expect("Failed to seed test session");
}

/// Executes a future within a real Axum session & auth context pipeline,
/// loading the session from the PostgreSQL database using the provided session token.
pub async fn with_test_session<F, R>(
    token: Option<&str>,
    fut_fn: impl FnOnce() -> F + Send + 'static,
) -> R
where
    F: std::future::Future<Output = R> + Send + 'static,
    R: Send + 'static,
{
    let pool = get_db().await.clone();
    let session_store = create_session_store(pool.clone()).await.unwrap();
    let auth_layer = create_auth_layer(pool);
    let session_layer = axum_session::SessionLayer::new(session_store);

    let (tx, rx) = tokio::sync::oneshot::channel();
    let tx = std::sync::Arc::new(std::sync::Mutex::new(Some(tx)));
    let fut_fn = std::sync::Arc::new(std::sync::Mutex::new(Some(fut_fn)));

    let service = ServiceBuilder::new()
        .layer(axum::middleware::from_fn(auth_header_to_cookie_middleware))
        .layer(session_layer)
        .layer(auth_layer)
        .service(tower::service_fn(move |req: Request<axum::body::Body>| {
            let fut = {
                let mut lock = fut_fn.lock().unwrap();
                lock.take().expect("test runner called once")()
            };
            let tx_clone = tx.clone();
            let (parts, _) = req.into_parts();
            let context = FullstackContext::new(parts);
            async move {
                let res = context.scope(fut).await;
                if let Some(sender) = tx_clone.lock().unwrap().take() {
                    let _ = sender.send(res);
                }
                Ok::<_, std::convert::Infallible>(axum::response::Response::new(
                    axum::body::Body::empty(),
                ))
            }
        }));

    let mut req_builder = Request::builder().uri("/api/test");
    if let Some(t) = token {
        req_builder = req_builder.header("cookie", format!("flowtask_session={t}"));
    }
    let req = req_builder.body(axum::body::Body::empty()).unwrap();

    let resp = service.oneshot(req).await;
    if let Err(e) = &resp {
        eprintln!("service.oneshot error: {:?}", e);
    }
    rx.await.expect("test handler completed")
}

/// Convenience helper to execute a future authenticated as a specific user ID,
/// backed by a real session in the PostgreSQL `sessions` table.
pub async fn run_as_user<F, R>(user_id: i64, fut_fn: impl FnOnce() -> F + Send + 'static) -> R
where
    F: std::future::Future<Output = R> + Send + 'static,
    R: Send + 'static,
{
    let token = format!("test_session_user_{user_id}");
    seed_test_session(user_id, &token).await;
    with_test_session(Some(&token), fut_fn).await
}
