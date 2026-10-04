#[cfg(feature = "server")]
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
#[cfg(feature = "server")]
use std::str::FromStr;
#[cfg(feature = "server")]
use crate::backend::crypto::hash_password;

#[cfg(feature = "server")]
static POOL: tokio::sync::OnceCell<sqlx::SqlitePool> = tokio::sync::OnceCell::const_new();

#[cfg(feature = "server")]
pub async fn get_db() -> &'static sqlx::SqlitePool {
    POOL.get_or_init(|| async {
        let _ = std::fs::create_dir_all("data");
        let options = SqliteConnectOptions::from_str("sqlite:data/todolist.db")
            .expect("Invalid SQLite connection string")
            .create_if_missing(true)
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .expect("Failed to connect to SQLite database");

        init_db(&pool).await.expect("Failed to initialize database schema");
        pool
    })
    .await
}

#[cfg(feature = "server")]
async fn init_db(pool: &sqlx::SqlitePool) -> Result<(), sqlx::Error> {
    // 1. Create base tables if they don't exist
    let base_tables_sql = "
        CREATE TABLE IF NOT EXISTS user (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL DEFAULT ''
        );
        CREATE TABLE IF NOT EXISTS session (
            token TEXT PRIMARY KEY,
            user_id INTEGER NOT NULL,
            created_at INTEGER NOT NULL,
            expires_at INTEGER NOT NULL,
            FOREIGN KEY(user_id) REFERENCES user(id) ON DELETE CASCADE
        );
        CREATE TABLE IF NOT EXISTS task (
            id INTEGER PRIMARY KEY,
            title TEXT NOT NULL,
            description TEXT,
            author INTEGER NOT NULL,
            status TEXT CHECK (status IN ('suggested', 'accepted', 'assigned', 'in_progress', 'finished')),
            FOREIGN KEY(author) REFERENCES user(id)
        );
        CREATE TABLE IF NOT EXISTS assignments (
            id INTEGER PRIMARY KEY,
            task_id INTEGER NOT NULL,
            assigned_to INTEGER NOT NULL,
            FOREIGN KEY(task_id) REFERENCES task(id) ON DELETE CASCADE,
            FOREIGN KEY(assigned_to) REFERENCES user(id)
        );
    ";
    sqlx::raw_sql(base_tables_sql).execute(pool).await?;

    // 2. Run migration for existing databases missing password_hash
    let _ = sqlx::query("ALTER TABLE user ADD COLUMN password_hash TEXT NOT NULL DEFAULT ''")
        .execute(pool)
        .await;

    // 3. Create triggers
    let triggers_sql = "
        -- Trigger 1: Cannot assign a task that is in 'suggested' status
        CREATE TRIGGER IF NOT EXISTS prevent_assign_suggested_task
        BEFORE INSERT ON assignments
        FOR EACH ROW
        WHEN (SELECT status FROM task WHERE id = NEW.task_id) = 'suggested'
        BEGIN
            SELECT RAISE(FAIL, 'Cannot assign a task that has suggested status');
        END;

        -- Trigger 2: Cannot change task status to 'in_progress' without at least one assignee
        CREATE TRIGGER IF NOT EXISTS enforce_in_progress_assigned
        BEFORE UPDATE OF status ON task
        FOR EACH ROW
        WHEN NEW.status = 'in_progress' AND NOT EXISTS (SELECT 1 FROM assignments WHERE task_id = NEW.id)
        BEGIN
            SELECT RAISE(FAIL, 'Cannot set status to in_progress without an assignee');
        END;

        -- Trigger 3: Cannot remove the last assignee if task is 'in_progress'
        CREATE TRIGGER IF NOT EXISTS prevent_unassign_in_progress_task
        BEFORE DELETE ON assignments
        FOR EACH ROW
        WHEN (SELECT status FROM task WHERE id = OLD.task_id) = 'in_progress' 
             AND (SELECT COUNT(*) FROM assignments WHERE task_id = OLD.task_id) <= 1
        BEGIN
            SELECT RAISE(FAIL, 'Cannot remove the last assignee from an in_progress task');
        END;
    ";
    sqlx::raw_sql(triggers_sql).execute(pool).await?;

    // 4. Seed initial users if not present
    let seed_users_sql = "
        INSERT OR IGNORE INTO user (id, name, email, password_hash) VALUES
            (1, 'Alice Johnson', 'alice@example.com', ''),
            (2, 'Bob Smith', 'bob@example.com', ''),
            (3, 'Charlie Brown', 'charlie@example.com', ''),
            (4, 'Diana Prince', 'diana@example.com', ''),
            (5, 'Evan Wright', 'evan@example.com', ''),
            (6, 'Fiona Gallagher', 'fiona@example.com', ''),
            (7, 'George Clark', 'george@example.com', ''),
            (8, 'Hannah Abbott', 'hannah@example.com', ''),
            (9, 'Ian Malcolm', 'ian@example.com', ''),
            (10, 'Julia Roberts', 'julia@example.com', ''),
            (11, 'Fiona Hill', 'fiona-hill@example.com', ''),
            (12, 'Hannah Montana', 'hannah.montana@disney.com', ''),
            (13, 'Ewan McGregor', 'ewan.mcgregor@coruscant.com', ''),
            (14, 'Gordon Brown', 'gordon-brown@gov.uk', ''),
            (15, 'George Best', 'george.best@manchester.united.com', '');
    ";
    sqlx::raw_sql(seed_users_sql).execute(pool).await?;

    // 5. Seed default password ('password123') for users with empty password_hash
    let default_hash = hash_password("password123").unwrap_or_default();
    if !default_hash.is_empty() {
        let _ = sqlx::query("UPDATE user SET password_hash = $1 WHERE password_hash = ''")
            .bind(&default_hash)
            .execute(pool)
            .await;
    }

    Ok(())
}