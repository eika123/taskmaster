use dioxus::prelude::*;

#[cfg(feature = "server")]
use rusqlite;
#[cfg(feature = "server")]
use rusqlite::params;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Suggested,
    Accepted,
    Assigned,
    InProgress,
    Finished,
}

impl TaskStatus {
    #[cfg(feature = "server")]
    pub const ALL: &'static [TaskStatus] = &[
        Self::Suggested,
        Self::Accepted,
        Self::Assigned,
        Self::InProgress,
        Self::Finished,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Suggested => "suggested",
            Self::Accepted => "accepted",
            Self::Assigned => "assigned",
            Self::InProgress => "in_progress",
            Self::Finished => "finished",
        }
    }

    /// Generates a comma-separated list of quoted status strings for SQL CHECK constraints:
    /// e.g., `'suggested', 'accepted', 'assigned', 'in_progress', 'finished'`
    #[cfg(feature = "server")]
    pub fn sql_check_constraint_list() -> String {
        Self::ALL
            .iter()
            .map(|s| format!("'{}'", s.as_str()))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
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

/// Serializes `TaskStatus` into an SQLite text parameter.
///
/// This writes the canonical `snake_case` string representation (e.g., `"in_progress"`)
/// directly to the database column, satisfying SQLite `CHECK` constraints.
#[cfg(feature = "server")]
impl rusqlite::ToSql for TaskStatus {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(rusqlite::types::ToSqlOutput::from(self.as_str()))
    }
}

/// Deserializes `TaskStatus` from an SQLite text column value.
///
/// Parses the text returned by SQLite using `TaskStatus::from_str`.
/// If the database column contains an unexpected or corrupt status string,
/// it produces a descriptive `FromSqlError::Other` conversion error.
#[cfg(feature = "server")]
impl rusqlite::types::FromSql for TaskStatus {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        value.as_str().and_then(|s| {
            s.parse::<TaskStatus>().map_err(|e| {
                rusqlite::types::FromSqlError::Other(Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    e,
                )))
            })
        })
    }
}

// The database is only available to server code
#[cfg(feature = "server")]
thread_local! {
    pub static DB: rusqlite::Connection = {
        // Open the database from the persisted "todolist.db" file
        let conn = rusqlite::Connection::open("todolist.db").expect("Failed to open database");

        // Enable foreign key constraint support in SQLite
        conn.execute_batch("PRAGMA foreign_keys = ON;").expect("Failed to enable foreign keys");

        // Create tables and triggers if they don't already exist.
        // The task.status CHECK constraint is dynamically derived from TaskStatus::ALL.
        let schema_sql = format!(
            "CREATE TABLE IF NOT EXISTS user (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                email TEXT NOT NULL UNIQUE
            );
            CREATE TABLE IF NOT EXISTS task (
                id INTEGER PRIMARY KEY,
                title TEXT NOT NULL,
                description TEXT,
                author INTEGER NOT NULL,
                status TEXT CHECK (status IN ({valid_statuses})),
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
            WHEN (SELECT status FROM task WHERE id = NEW.task_id) = '{suggested}'
            BEGIN
                SELECT RAISE(FAIL, 'Cannot assign a task that has suggested status');
            END;

            -- Trigger 2: Cannot change task status to 'in_progress' without at least one assignee
            CREATE TRIGGER IF NOT EXISTS enforce_in_progress_assigned
            BEFORE UPDATE OF status ON task
            FOR EACH ROW
            WHEN NEW.status = '{in_progress}' AND NOT EXISTS (SELECT 1 FROM assignments WHERE task_id = NEW.id)
            BEGIN
                SELECT RAISE(FAIL, 'Cannot set status to in_progress without an assignee');
            END;

            -- Trigger 3: Cannot remove the last assignee if task is 'in_progress'
            CREATE TRIGGER IF NOT EXISTS prevent_unassign_in_progress_task
            BEFORE DELETE ON assignments
            FOR EACH ROW
            WHEN (SELECT status FROM task WHERE id = OLD.task_id) = '{in_progress}' 
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
            ",
            valid_statuses = TaskStatus::sql_check_constraint_list(),
            suggested = TaskStatus::Suggested.as_str(),
            in_progress = TaskStatus::InProgress.as_str(),
        );

        conn.execute_batch(&schema_sql).unwrap();

        // Return the connection
        conn
    };
}

#[server]
pub async fn suggest_task(
    user_email: String,
    task_title: String,
    description: String,
) -> Result<(), ServerFnError> {
    DB.with(|conn| {
        conn.execute(
            "INSERT INTO task (title, description, author, status)
             VALUES (?1, ?2, (SELECT id FROM user WHERE email = ?3), ?4)",
            params![task_title, description, user_email, TaskStatus::Suggested],
        )
    })
    .map_err(|e| ServerFnError::new(format!("Failed to create suggested task: {e}")))?;

    Ok(())
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
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

#[server]
pub async fn get_users() -> Result<Vec<User>, ServerFnError> {
    DB.with(|conn| {
        let mut statement = conn.prepare("SELECT id, name, email FROM user ORDER BY name ASC")?;
        let user_iter = statement.query_map([], |row| {
            Ok(User {
                id: row.get(0)?,
                name: row.get(1)?,
                email: row.get(2)?,
            })
        })?;

        let mut users = Vec::new();
        for user in user_iter {
            users.push(user?);
        }
        Ok(users)
    })
    .map_err(|e: rusqlite::Error| ServerFnError::new(format!("Failed to fetch users: {e}")))
}

#[server]
pub async fn assign_task(task_id: i64, user_id: i64) -> Result<(), ServerFnError> {
    DB.with(|conn| {
        conn.execute(
            "INSERT OR IGNORE INTO assignments (task_id, assigned_to) VALUES (?1, ?2)",
            params![task_id, user_id],
        )
    })
    .map_err(|e| ServerFnError::new(format!("Failed to assign task: {e}")))?;

    Ok(())
}

#[server]
pub async fn unassign_task(task_id: i64, user_id: i64) -> Result<(), ServerFnError> {
    DB.with(|conn| {
        conn.execute(
            "DELETE FROM assignments WHERE task_id = ?1 AND assigned_to = ?2",
            params![task_id, user_id],
        )
    })
    .map_err(|e| ServerFnError::new(format!("Failed to unassign task: {e}")))?;

    Ok(())
}

#[server]
pub async fn get_all_tasks(after_id: Option<i64>, amount: u64) -> Result<Vec<Task>, ServerFnError> {
    let last_id = after_id.unwrap_or(0);
    DB.with(|conn| {
        let mut statement = conn.prepare(
            "SELECT task.id, task.title, task.author, user.name, task.description, task.status 
             FROM task 
             INNER JOIN user ON task.author = user.id 
             WHERE task.id > ?1 
             ORDER BY task.id ASC 
             LIMIT ?2",
        )?;

        let mut raw_tasks = Vec::new();
        let task_iter = statement.query_map(params![last_id, amount], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, TaskStatus>(5)?,
            ))
        })?;

        for task in task_iter {
            raw_tasks.push(task?);
        }

        let mut tasks = Vec::new();
        let mut assignees_stmt = conn.prepare(
            "SELECT user.id, user.name, user.email 
             FROM assignments 
             INNER JOIN user ON assignments.assigned_to = user.id 
             WHERE assignments.task_id = ?1 
             ORDER BY user.name ASC",
        )?;

        for (id, title, author, author_name, description, status) in raw_tasks {
            let user_iter = assignees_stmt.query_map(params![id], |row| {
                Ok(User {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    email: row.get(2)?,
                })
            })?;

            let mut assignees = Vec::new();
            for u in user_iter {
                assignees.push(u?);
            }

            tasks.push(Task {
                id,
                title,
                author,
                author_name,
                description,
                status,
                assignees,
            });
        }

        Ok(tasks)
    })
    .map_err(|e: rusqlite::Error| ServerFnError::new(format!("Failed to fetch tasks: {e}")))
}

#[server]
pub async fn get_task(task_id: i64) -> Result<Task, ServerFnError> {
    DB.with(|conn| {
        let mut statement = conn.prepare(
            "SELECT task.id, task.title, task.author, user.name, task.description, task.status 
             FROM task 
             INNER JOIN user ON task.author = user.id 
             WHERE task.id = ?1",
        )?;

        let (id, title, author, author_name, description, status) = statement.query_row(params![task_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, TaskStatus>(5)?,
            ))
        })?;

        let mut assignees_stmt = conn.prepare(
            "SELECT user.id, user.name, user.email 
             FROM assignments 
             INNER JOIN user ON assignments.assigned_to = user.id 
             WHERE assignments.task_id = ?1 
             ORDER BY user.name ASC",
        )?;

        let user_iter = assignees_stmt.query_map(params![id], |row| {
            Ok(User {
                id: row.get(0)?,
                name: row.get(1)?,
                email: row.get(2)?,
            })
        })?;

        let mut assignees = Vec::new();
        for u in user_iter {
            assignees.push(u?);
        }

        Ok(Task {
            id,
            title,
            author,
            author_name,
            description,
            status,
            assignees,
        })
    })
    .map_err(|e: rusqlite::Error| ServerFnError::new(format!("Failed to fetch task: {e}")))
}

#[server]
pub async fn delete_task(task_id: i64) -> Result<(), ServerFnError> {
    DB.with(|conn| conn.execute("DELETE FROM task WHERE id = ?1", params![task_id]))
        .map_err(|e| ServerFnError::new(format!("Failed to delete task: {e}")))?;

    Ok(())
}

#[server]
pub async fn set_task_status(task_id: u64, status: TaskStatus) -> Result<(), ServerFnError> {
    DB.with(|conn| {
        conn.execute(
            "UPDATE task SET status=?1 WHERE id=?2",
            params![status, task_id],
        )
    })
    .map_err(|e| ServerFnError::new(format!("Failed to update task: {e}")))?;
    Ok(())
}

