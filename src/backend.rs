use dioxus::prelude::*;

#[cfg(feature = "server")]
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
#[cfg(feature = "server")]
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "server", derive(sqlx::Type))]
#[cfg_attr(feature = "server", sqlx(rename_all = "snake_case"))]
pub enum TaskStatus {
    Suggested,
    Accepted,
    Assigned,
    InProgress,
    Finished,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Suggested => "suggested",
            Self::Accepted => "accepted",
            Self::Assigned => "assigned",
            Self::InProgress => "in_progress",
            Self::Finished => "finished",
        }
    }
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for TaskStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "suggested" => Ok(Self::Suggested),
            "accepted" => Ok(Self::Accepted),
            "assigned" => Ok(Self::Assigned),
            "in_progress" => Ok(Self::InProgress),
            "finished" => Ok(Self::Finished),
            other => Err(format!("Unknown task status: '{other}'")),
        }
    }
}

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
    let schema_sql = "
        CREATE TABLE IF NOT EXISTS user (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL UNIQUE
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

        -- Seed initial users if not present
        INSERT OR IGNORE INTO user (id, name, email) VALUES
            (1, 'Alice Johnson', 'alice@example.com'),
            (2, 'Bob Smith', 'bob@example.com'),
            (3, 'Charlie Brown', 'charlie@example.com'),
            (4, 'Diana Prince', 'diana@example.com'),
            (5, 'Evan Wright', 'evan@example.com'),
            (6, 'Fiona Gallagher', 'fiona@example.com'),
            (7, 'George Clark', 'george@example.com'),
            (8, 'Hannah Abbott', 'hannah@example.com'),
            (9, 'Ian Malcolm', 'ian@example.com'),
            (10, 'Julia Roberts', 'julia@example.com'),
            (11, 'Fiona Hill', 'fiona-hill@example.com'),
            (12, 'Hannah Montana', 'hannah.montana@disney.com'),
            (13, 'Ewan McGregor', 'ewan.mcgregor@coruscant.com'),
            (14, 'Gordon Brown', 'gordon-brown@gov.uk'),
            (15, 'George Best', 'george.best@manchester.united.com');
    ";


    sqlx::raw_sql(&schema_sql).execute(pool).await?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct User {
    pub id: i64,
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Task {
    pub id: i64,
    pub title: String,
    pub author: i64,
    pub author_name: String,
    pub description: String,
    pub status: TaskStatus,
    pub assignees: Vec<User>,
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
    user_email: String,
    task_title: String,
    description: String,
) -> Result<(), ServerFnError> {
    let pool = get_db().await;
    sqlx::query(
        "INSERT INTO task (title, description, author, status)
         VALUES ($1, $2, (SELECT id FROM user WHERE email = $3), $4)",
    )
    .bind(task_title)
    .bind(description)
    .bind(user_email)
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
