use dioxus::prelude::*;

#[cfg(feature = "server")]
use rusqlite::params;
#[cfg(feature = "server")]
use rusqlite;

// The database is only available to server code
#[cfg(feature = "server")]
thread_local! {
    pub static DB: rusqlite::Connection = {
        // Open the database from the persisted "hotdog.db" file
        let conn = rusqlite::Connection::open("todolist.db").expect("Failed to open database");

        // Create the "dogs" table if it doesn't already exist
        conn.execute_batch(
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
                status TEXT CHECK (status IN ('suggested', 'accepted', 'assigned', 'in_progress', 'finished')),
                FOREIGN KEY(author) REFERENCES user(id)
            );
            CREATE TABLE IF NOT EXISTS assignments (
                id INTEGER PRIMARY KEY,
                task_id INTEGER NOT NULL,
                assigned_to INTEGER NOT NULL,
                FOREIGN KEY(task_id) REFERENCES task(id),
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

            -- Seed dummy user if not present
            INSERT OR IGNORE INTO user (id, name, email) VALUES (1, 'Dummy User', 'dummy@example.com');
            ",
        ).unwrap();

        // Return the connection
        conn
    };
}

#[server]
pub async fn suggest_task(user_email: String, task_title: String, description: String) -> Result<(), ServerFnError> {
    DB.with(|conn| {
        conn.execute(
            "INSERT INTO task (title, description, author, status)
             VALUES (?1, ?2, (SELECT id FROM user WHERE email = ?3), 'suggested')",
            params![task_title, description, user_email],
        )
    })
    .map_err(|e| ServerFnError::new(format!("Failed to create suggested task: {e}")))?;

    Ok(())
}


#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Task {
    pub id: i64,
    pub title: String,
    pub author: i64,
    pub description: String,
    pub status: String,
}

#[server]
pub async fn get_all_tasks(after_id: Option<i64>, amount: u64) -> Result<Vec<Task>, ServerFnError> {
    let last_id = after_id.unwrap_or(0);
    DB.with(|conn| {
        let mut statement = conn.prepare(
            "SELECT id, title, author, description, status FROM task WHERE id > ?1 ORDER BY id ASC LIMIT ?2",
        )?;
        let task_iter = statement.query_map(params![last_id, amount], |row| {
            Ok(Task {
                id: row.get(0)?,
                title: row.get(1)?,
                author: row.get(2)?,
                description: row.get(3)?,
                status: row.get(4)?,
            })
        })?;

        let mut tasks = Vec::new();
        for task in task_iter {
            tasks.push(task?);
        }
        Ok(tasks)
    })
    .map_err(|e: rusqlite::Error| ServerFnError::new(format!("Failed to fetch tasks: {e}")))
}

#[server]
pub async fn delete_task(task_id: i64) -> Result<(), ServerFnError> {
    DB.with(|conn| {
        conn.execute("DELETE FROM task WHERE id = ?1", params![task_id])
    })
    .map_err(|e| ServerFnError::new(format!("Failed to delete task: {e}")))?;

    Ok(())
}

