#[cfg(feature = "server")]
use crate::backend::crypto::hash_password;
#[cfg(feature = "server")]
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
#[cfg(feature = "server")]
use std::str::FromStr;

#[cfg(feature = "server")]
pub fn load_env_files() {
    for path in ["data/.env", ".env"] {
        if let Ok(content) = std::fs::read_to_string(path) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }
                if let Some((key, val)) = trimmed.split_once('=') {
                    let key = key.trim();
                    let val = val.trim().trim_matches('"').trim_matches('\'');
                    if std::env::var_os(key).is_none() {
                        std::env::set_var(key, val);
                    }
                }
            }
        }
    }
}

#[cfg(feature = "server")]
pub fn resolve_database_url() -> String {
    load_env_files();
    if let Ok(url) = std::env::var("DATABASE_URL") {
        if !url.trim().is_empty() {
            return url;
        }
    }
    let user = std::env::var("POSTGRES_USER").unwrap_or_else(|_| "flowtask".to_string());
    let password =
        std::env::var("POSTGRES_PASSWORD").unwrap_or_else(|_| "flowtask_dev_password".to_string());
    let host = std::env::var("POSTGRES_HOST").unwrap_or_else(|_| "localhost".to_string());
    let port = std::env::var("POSTGRES_PORT").unwrap_or_else(|_| "5432".to_string());
    let db = std::env::var("POSTGRES_DB").unwrap_or_else(|_| "flowtask_db".to_string());
    format!("postgres://{user}:{password}@{host}:{port}/{db}")
}

#[cfg(all(feature = "server", not(test)))]
static POOL: tokio::sync::OnceCell<sqlx::PgPool> = tokio::sync::OnceCell::const_new();

#[cfg(all(feature = "server", test))]
static TEST_SCHEMA_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);

#[cfg(all(feature = "server", test))]
static CLEANUP_OLD_TEST_SCHEMAS: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();

#[cfg(all(feature = "server", test))]
std::thread_local! {
    static TEST_POOL: std::cell::Cell<Option<&'static sqlx::PgPool>> = const { std::cell::Cell::new(None) };
}

#[cfg(all(feature = "server", test))]
fn get_cached_test_pool() -> Option<&'static sqlx::PgPool> {
    TEST_POOL.with(|c| c.get())
}

#[cfg(all(feature = "server", test))]
fn set_cached_test_pool(pool: &'static sqlx::PgPool) {
    TEST_POOL.with(|c| c.set(Some(pool)));
}

#[cfg(all(feature = "server", not(test)))]
async fn connect_pg_pool(max_connections: u32) -> sqlx::PgPool {
    let url = resolve_database_url();
    let options = PgConnectOptions::from_str(&url).expect("Invalid PostgreSQL DATABASE_URL");

    PgPoolOptions::new()
        .max_connections(max_connections)
        .acquire_timeout(std::time::Duration::from_secs(15))
        .connect_with(options)
        .await
        .expect("Failed to connect to PostgreSQL database")
}

#[cfg(all(feature = "server", not(test)))]
pub async fn get_db() -> &'static sqlx::PgPool {
    POOL.get_or_init(|| async {
        let pool = connect_pg_pool(10).await;
        init_db(&pool)
            .await
            .expect("Failed to initialize PostgreSQL schema");
        pool
    })
    .await
}

#[cfg(all(feature = "server", test))]
pub async fn get_db() -> &'static sqlx::PgPool {
    if let Some(pool) = get_cached_test_pool() {
        return pool;
    }
    let url = resolve_database_url();
    CLEANUP_OLD_TEST_SCHEMAS
        .get_or_init(|| async {
            if let Ok(opts) = PgConnectOptions::from_str(&url) {
                if let Ok(mut conn) = sqlx::ConnectOptions::connect(&opts).await {
                    use sqlx::Executor;
                    let _ = conn
                        .execute(
                            r#"
                            DO $$
                            DECLARE r RECORD;
                            BEGIN
                                FOR r IN (
                                    SELECT schema_name FROM information_schema.schemata
                                    WHERE schema_name LIKE 'test_s_%'
                                ) LOOP
                                    EXECUTE 'DROP SCHEMA IF EXISTS ' || quote_ident(r.schema_name) || ' CASCADE';
                                END LOOP;
                            END $$;
                            "#,
                        )
                        .await;
                }
            }
        })
        .await;

    let idx = TEST_SCHEMA_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let schema = format!("test_s_{}_{}", std::process::id(), idx);
    let options = PgConnectOptions::from_str(&url).expect("Invalid PostgreSQL DATABASE_URL");

    let schema_for_hook = schema.clone();
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(std::time::Duration::from_secs(15))
        .after_connect(move |conn, _meta| {
            let s = schema_for_hook.clone();
            Box::pin(async move {
                use sqlx::Executor;
                conn.execute(
                    format!("CREATE SCHEMA IF NOT EXISTS {s}; SET search_path TO {s};").as_str(),
                )
                .await?;
                Ok(())
            })
        })
        .connect_with(options)
        .await
        .expect("Failed to connect to PostgreSQL test database");

    init_db_inner(&pool)
        .await
        .expect("Failed to initialize isolated PostgreSQL test schema");

    let leaked: &'static sqlx::PgPool = Box::leak(Box::new(pool));
    set_cached_test_pool(leaked);
    leaked
}

#[cfg(all(feature = "server", not(test)))]
async fn init_db(pool: &sqlx::PgPool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut conn = pool.acquire().await?;
    sqlx::query("SELECT pg_advisory_lock(74839201)")
        .execute(&mut *conn)
        .await?;
    let res = init_db_inner(pool).await;
    let _ = sqlx::query("SELECT pg_advisory_unlock(74839201)")
        .execute(&mut *conn)
        .await;
    res
}

#[cfg(feature = "server")]
async fn init_db_inner(
    pool: &sqlx::PgPool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    sqlx::migrate!("./migrations").run(pool).await?;

    // Seed default password ('password123') for seeded demo users
    let default_hash = hash_password("password123").unwrap_or_default();
    if !default_hash.is_empty() {
        let _ = sqlx::query("UPDATE \"user\" SET password_hash = $1 WHERE password_hash = ''")
            .bind(&default_hash)
            .execute(pool)
            .await;
    }

    Ok(())
}
