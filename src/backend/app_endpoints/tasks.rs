use dioxus::prelude::*;
#[allow(unused_imports)]
use crate::types::{Permission, Task, TaskMessage, TaskStatus, User};

#[cfg(feature = "server")]
use crate::backend::app_endpoints::auth::get_auth_user;
#[cfg(feature = "server")]
use crate::backend::database::get_db;

#[cfg(feature = "server")]
#[derive(sqlx::FromRow)]
pub(crate) struct RawTask {
    pub id: i64,
    pub project_id: i64,
    pub project_name: Option<String>,
    pub title: String,
    pub author: Option<i64>,
    pub author_name: Option<String>,
    pub description: Option<String>,
    pub status: TaskStatus,
}

#[server]
pub async fn suggest_task(
    task_title: String,
    description: String,
) -> Result<(), ServerFnError> {
    suggest_task_for_project(1, task_title, description).await
}

#[server]
pub async fn suggest_task_for_project(
    project_id: i64,
    task_title: String,
    description: String,
) -> Result<(), ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;

    let can_create: bool = sqlx::query_scalar(
        "SELECT (p.leader_id = $1 OR EXISTS (
            SELECT 1 FROM project_members pm
            WHERE pm.project_id = p.id AND pm.user_id = $1 AND has_perm(pm.permissions_mask, $3)
         ))
         FROM project p WHERE p.id = $2",
    )
    .bind(user.id)
    .bind(project_id)
    .bind(Permission::EXECUTE.as_i64())
    .fetch_optional(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?
    .unwrap_or(false);

    if !can_create {
        return Err(ServerFnError::new(
            "You do not have permission to create or suggest tasks in this project.",
        ));
    }

    sqlx::query(
        "INSERT INTO task (project_id, title, description, author, status) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(project_id)
    .bind(task_title.trim())
    .bind(description.trim())
    .bind(user.id)
    .bind(TaskStatus::Suggested)
    .execute(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to create suggested task: {e}")))?;

    Ok(())
}

#[server]
pub async fn assign_task(task_id: i64, user_id: i64) -> Result<(), ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;

    let project_id: Option<i64> = sqlx::query_scalar("SELECT project_id FROM task WHERE id = $1")
        .bind(task_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ServerFnError::new(format!("Task not found: {e}")))?
        .flatten();

    if let Some(pid) = project_id {
        let can_edit: bool = sqlx::query_scalar(
            "SELECT (p.leader_id = $1 OR EXISTS (
                SELECT 1 FROM project_members pm
                WHERE pm.project_id = p.id AND pm.user_id = $1 AND has_perm(pm.permissions_mask, $3)
             ))
             FROM project p WHERE p.id = $2",
        )
        .bind(user.id)
        .bind(pid)
        .bind(Permission::EXECUTE.as_i64())
        .fetch_optional(pool)
        .await
        .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?
        .unwrap_or(false);

        if !can_edit {
            return Err(ServerFnError::new(
                "You do not have permission to manage assignments in this project.",
            ));
        }

        let assignee_valid: bool = sqlx::query_scalar(
            "SELECT (p.leader_id = $1 OR EXISTS (
                SELECT 1 FROM project_members pm
                WHERE pm.project_id = p.id AND pm.user_id = $1 AND has_perm(pm.permissions_mask, $3)
             ))
             FROM project p WHERE p.id = $2",
        )
        .bind(user_id)
        .bind(pid)
        .bind(Permission::EXECUTE.as_i64())
        .fetch_optional(pool)
        .await
        .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?
        .unwrap_or(false);

        if !assignee_valid {
            return Err(ServerFnError::new(
                "Cannot assign task to a user who lacks task execution permissions in this project.",
            ));
        }
    }

    sqlx::query("INSERT INTO assignments (task_id, assigned_to) VALUES ($1, $2) ON CONFLICT DO NOTHING")
        .bind(task_id)
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to assign task: {e}")))?;

    Ok(())
}

#[server]
pub async fn unassign_task(task_id: i64, user_id: i64) -> Result<(), ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;

    let project_id: Option<i64> = sqlx::query_scalar("SELECT project_id FROM task WHERE id = $1")
        .bind(task_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ServerFnError::new(format!("Task not found: {e}")))?
        .flatten();

    if let Some(pid) = project_id {
        let can_edit: bool = sqlx::query_scalar(
            "SELECT (p.leader_id = $1 OR EXISTS (
                SELECT 1 FROM project_members pm
                WHERE pm.project_id = p.id AND pm.user_id = $1 AND has_perm(pm.permissions_mask, $3)
             ))
             FROM project p WHERE p.id = $2",
        )
        .bind(user.id)
        .bind(pid)
        .bind(Permission::EXECUTE.as_i64())
        .fetch_optional(pool)
        .await
        .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?
        .unwrap_or(false);

        if !can_edit {
            return Err(ServerFnError::new(
                "You do not have permission to manage assignments in this project.",
            ));
        }
    }

    sqlx::query("DELETE FROM assignments WHERE task_id = $1 AND assigned_to = $2")
        .bind(task_id)
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to unassign task: {e}")))?;

    Ok(())
}

#[server]
pub async fn get_my_tasks(
    after_id: Option<i64>,
    amount: u64,
) -> Result<Vec<Task>, ServerFnError> {
    let user = get_auth_user().await?;
    let last_id = after_id.unwrap_or(0);
    let pool = get_db().await;

    let raw_tasks = sqlx::query_as::<_, RawTask>(
        "SELECT DISTINCT task.id, task.project_id, COALESCE(project.name, 'Standalone') AS project_name,
                task.title, task.author,
                COALESCE(u_author.name, 'Deleted User') AS author_name, 
                task.description, task.status 
         FROM task 
         LEFT JOIN project ON task.project_id = project.id
         LEFT JOIN \"user\" u_author ON task.author = u_author.id 
         LEFT JOIN assignments ON task.id = assignments.task_id
         WHERE (assignments.assigned_to = $1 OR task.author = $1)
           AND (
               task.project_id IS NULL
               OR task.project_id = 0
               OR project.leader_id = $1
               OR EXISTS (
                   SELECT 1 FROM project_members pm
                   WHERE pm.project_id = task.project_id
                     AND pm.user_id = $1
                     AND has_perm(pm.permissions_mask, $4)
               )
           )
           AND task.id > $2 
         ORDER BY task.id ASC 
         LIMIT $3",
    )
    .bind(user.id)
    .bind(last_id)
    .bind(amount as i64)
    .bind(Permission::READ.as_i64())
    .fetch_all(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to fetch my tasks: {e}")))?;

    if raw_tasks.is_empty() {
        return Ok(Vec::new());
    }

    let task_ids: Vec<i64> = raw_tasks.iter().map(|t| t.id).collect();
    let query_str = format!(
        "SELECT assignments.task_id, u.id, u.name, u.email 
         FROM assignments 
         INNER JOIN \"user\" u ON assignments.assigned_to = u.id 
         WHERE assignments.task_id IN ({}) 
         ORDER BY u.name ASC",
        task_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",")
    );

    #[derive(sqlx::FromRow)]
    struct RawAssignment {
        task_id: i64,
        id: i64,
        name: String,
        email: String,
    }

    let assignments_rows = sqlx::query_as::<_, RawAssignment>(&query_str)
        .fetch_all(pool)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to batch fetch assignees: {e}")))?;

    use std::collections::HashMap;
    let mut assignees_map: HashMap<i64, Vec<User>> = HashMap::new();
    for row in assignments_rows {
        assignees_map.entry(row.task_id).or_default().push(User {
            id: row.id,
            name: row.name,
            email: row.email,
        });
    }

    let tasks = raw_tasks
        .into_iter()
        .map(|raw| {
            let assignees = assignees_map.remove(&raw.id).unwrap_or_default();
            Task {
                id: raw.id,
                project_id: raw.project_id,
                project_name: raw.project_name.unwrap_or_else(|| "Standalone".to_string()),
                title: raw.title,
                author: raw.author,
                author_name: raw.author_name.unwrap_or_else(|| "Deleted User".to_string()),
                description: raw.description.unwrap_or_default(),
                status: raw.status,
                assignees,
            }
        })
        .collect();

    Ok(tasks)
}

#[server]
pub async fn get_all_tasks(
    after_id: Option<i64>,
    amount: u64,
) -> Result<Vec<Task>, ServerFnError> {
    get_project_tasks(1, after_id, amount).await
}

#[server]
pub async fn get_project_tasks(
    project_id: i64,
    after_id: Option<i64>,
    amount: u64,
) -> Result<Vec<Task>, ServerFnError> {
    let user = get_auth_user().await?;
    let last_id = after_id.unwrap_or(0);
    let pool = get_db().await;

    let can_read: bool = sqlx::query_scalar(
        "SELECT (p.leader_id = $1 OR EXISTS (
            SELECT 1 FROM project_members pm
            WHERE pm.project_id = p.id AND pm.user_id = $1 AND has_perm(pm.permissions_mask, $3)
         ))
         FROM project p WHERE p.id = $2",
    )
    .bind(user.id)
    .bind(project_id)
    .bind(Permission::READ.as_i64())
    .fetch_optional(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?
    .unwrap_or(false);

    if !can_read {
        return Err(ServerFnError::new(
            "You do not have permission to view this project's tasks.",
        ));
    }

    let raw_tasks = sqlx::query_as::<_, RawTask>(
        "SELECT task.id, task.project_id, COALESCE(project.name, 'Standalone') AS project_name,
                task.title, task.author,
                COALESCE(u.name, 'Deleted User') AS author_name, 
                task.description, task.status 
         FROM task 
         LEFT JOIN project ON task.project_id = project.id
         LEFT JOIN \"user\" u ON task.author = u.id 
         WHERE task.project_id = $1 AND task.id > $2 
         ORDER BY task.id ASC 
         LIMIT $3",
    )
    .bind(project_id)
    .bind(last_id)
    .bind(amount as i64)
    .fetch_all(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to fetch tasks: {e}")))?;

    if raw_tasks.is_empty() {
        return Ok(Vec::new());
    }

    let task_ids: Vec<i64> = raw_tasks.iter().map(|t| t.id).collect();
    let query_str = format!(
        "SELECT assignments.task_id, u.id, u.name, u.email 
         FROM assignments 
         INNER JOIN \"user\" u ON assignments.assigned_to = u.id 
         WHERE assignments.task_id IN ({}) 
         ORDER BY u.name ASC",
        task_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",")
    );

    #[derive(sqlx::FromRow)]
    struct RawAssignment {
        task_id: i64,
        id: i64,
        name: String,
        email: String,
    }

    let assignments_rows = sqlx::query_as::<_, RawAssignment>(&query_str)
        .fetch_all(pool)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to batch fetch assignees: {e}")))?;

    use std::collections::HashMap;
    let mut assignees_map: HashMap<i64, Vec<User>> = HashMap::new();
    for row in assignments_rows {
        assignees_map.entry(row.task_id).or_default().push(User {
            id: row.id,
            name: row.name,
            email: row.email,
        });
    }

    let tasks = raw_tasks
        .into_iter()
        .map(|raw| {
            let assignees = assignees_map.remove(&raw.id).unwrap_or_default();
            Task {
                id: raw.id,
                project_id: raw.project_id,
                project_name: raw.project_name.unwrap_or_else(|| "Standalone".to_string()),
                title: raw.title,
                author: raw.author,
                author_name: raw.author_name.unwrap_or_else(|| "Deleted User".to_string()),
                description: raw.description.unwrap_or_default(),
                status: raw.status,
                assignees,
            }
        })
        .collect();

    Ok(tasks)
}

#[server]
pub async fn get_task(task_id: i64) -> Result<Task, ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;

    let raw = sqlx::query_as::<_, RawTask>(
        "SELECT task.id, task.project_id, COALESCE(project.name, 'Standalone') AS project_name,
                task.title, task.author,
                COALESCE(u.name, 'Deleted User') AS author_name, 
                task.description, task.status 
         FROM task 
         LEFT JOIN project ON task.project_id = project.id
         LEFT JOIN \"user\" u ON task.author = u.id 
         WHERE task.id = $1",
    )
    .bind(task_id)
    .fetch_one(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to fetch task: {e}")))?;

    if raw.project_id != 0 {
        let can_read: bool = sqlx::query_scalar(
            "SELECT (p.leader_id = $1 OR EXISTS (
                SELECT 1 FROM project_members pm
                WHERE pm.project_id = p.id AND pm.user_id = $1 AND has_perm(pm.permissions_mask, $3)
             ))
             FROM project p WHERE p.id = $2",
        )
        .bind(user.id)
        .bind(raw.project_id)
        .bind(Permission::READ.as_i64())
        .fetch_optional(pool)
        .await
        .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?
        .unwrap_or(false);

        if !can_read {
            return Err(ServerFnError::new(
                "You do not have permission to view this task.",
            ));
        }
    }

    let assignees = sqlx::query_as::<_, User>(
        "SELECT u.id, u.name, u.email 
         FROM assignments 
         INNER JOIN \"user\" u ON assignments.assigned_to = u.id 
         WHERE assignments.task_id = $1 
         ORDER BY u.name ASC",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to fetch task assignees: {e}")))?;

    Ok(Task {
        id: raw.id,
        project_id: raw.project_id,
        project_name: raw.project_name.unwrap_or_else(|| "Standalone".to_string()),
        title: raw.title,
        author: raw.author,
        author_name: raw.author_name.unwrap_or_else(|| "Deleted User".to_string()),
        description: raw.description.unwrap_or_default(),
        status: raw.status,
        assignees,
    })
}

#[server]
pub async fn update_task(
    task_id: i64,
    title: String,
    description: String,
    status: TaskStatus,
    assignee_ids: Vec<i64>,
) -> Result<Task, ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ServerFnError::new(format!("Transaction failed: {e}")))?;

    let task_meta: Option<(i64, Option<i64>)> = sqlx::query_as(
        "SELECT project_id, author FROM task WHERE id = $1",
    )
    .bind(task_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?;

    let (project_id, author) = match task_meta {
        Some(m) => m,
        None => return Err(ServerFnError::new("Task not found.")),
    };

    if project_id != 0 {
        let can_edit: bool = sqlx::query_scalar(
            "SELECT (p.leader_id = $1 OR EXISTS (
                SELECT 1 FROM project_members pm
                WHERE pm.project_id = p.id AND pm.user_id = $1 AND has_perm(pm.permissions_mask, $3)
             ))
             FROM project p WHERE p.id = $2",
        )
        .bind(user.id)
        .bind(project_id)
        .bind(Permission::EXECUTE.as_i64())
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?
        .unwrap_or(false);

        if !can_edit {
            return Err(ServerFnError::new(
                "You do not have permission to edit tasks in this project.",
            ));
        }

        for uid in &assignee_ids {
            let assignee_valid: bool = sqlx::query_scalar(
                "SELECT (p.leader_id = $1 OR EXISTS (
                    SELECT 1 FROM project_members pm
                    WHERE pm.project_id = p.id AND pm.user_id = $1 AND has_perm(pm.permissions_mask, $3)
                 ))
                 FROM project p WHERE p.id = $2",
            )
            .bind(*uid)
            .bind(project_id)
            .bind(Permission::EXECUTE.as_i64())
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?
            .unwrap_or(false);

            if !assignee_valid {
                return Err(ServerFnError::new(
                    "Cannot assign task to a user who lacks task execution permissions in this project.",
                ));
            }
        }
    } else {
        if author != Some(user.id) {
            let is_assigned: bool = sqlx::query_scalar(
                "SELECT COUNT(*) > 0 FROM assignments WHERE task_id = $1 AND assigned_to = $2",
            )
            .bind(task_id)
            .bind(user.id)
            .fetch_one(&mut *tx)
            .await
            .unwrap_or(false);

            if !is_assigned {
                return Err(ServerFnError::new(
                    "You do not have permission to edit this standalone task.",
                ));
            }
        }
    }

    if status == TaskStatus::Suggested && !assignee_ids.is_empty() {
        return Err(ServerFnError::new(
            "Cannot assign users to a task in 'suggested' status.",
        ));
    }
    if status == TaskStatus::InProgress && assignee_ids.is_empty() {
        return Err(ServerFnError::new(
            "Cannot set status to 'in_progress' without at least one assignee.",
        ));
    }

    sqlx::query("UPDATE task SET title = $1, description = $2 WHERE id = $3")
        .bind(title.trim())
        .bind(description.trim())
        .bind(task_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to update task details: {e}")))?;

    let current_status: TaskStatus = sqlx::query_scalar("SELECT status FROM task WHERE id = $1")
        .bind(task_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to fetch current status: {e}")))?;

    if current_status == TaskStatus::InProgress && status != TaskStatus::InProgress {
        sqlx::query("UPDATE task SET status = 'accepted' WHERE id = $1")
            .bind(task_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| ServerFnError::new(format!("Failed to stage status: {e}")))?;
    }

    for user_id in &assignee_ids {
        sqlx::query("INSERT INTO assignments (task_id, assigned_to) VALUES ($1, $2) ON CONFLICT DO NOTHING")
            .bind(task_id)
            .bind(*user_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| ServerFnError::new(format!("Failed to add assignee: {e}")))?;
    }

    if assignee_ids.is_empty() {
        sqlx::query("DELETE FROM assignments WHERE task_id = $1")
            .bind(task_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| ServerFnError::new(format!("Failed to clear assignments: {e}")))?;
    } else {
        let keep_ids = assignee_ids
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let delete_sql = format!(
            "DELETE FROM assignments WHERE task_id = $1 AND assigned_to NOT IN ({})",
            keep_ids
        );
        sqlx::query(&delete_sql)
            .bind(task_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| ServerFnError::new(format!("Failed to remove old assignees: {e}")))?;
    }

    sqlx::query("UPDATE task SET status = $1 WHERE id = $2")
        .bind(status)
        .bind(task_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to update status: {e}")))?;

    tx.commit()
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to commit task update: {e}")))?;

    get_task(task_id).await
}

#[server]
pub async fn delete_task(task_id: i64) -> Result<(), ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;

    let task_meta: Option<(i64, Option<i64>)> = sqlx::query_as(
        "SELECT project_id, author FROM task WHERE id = $1",
    )
    .bind(task_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?;

    let (project_id, author) = match task_meta {
        Some(m) => m,
        None => return Err(ServerFnError::new("Task not found.")),
    };

    if project_id != 0 {
        let can_delete: bool = sqlx::query_scalar(
            "SELECT (p.leader_id = $1 OR EXISTS (
                SELECT 1 FROM project_members pm
                WHERE pm.project_id = p.id AND pm.user_id = $1 AND has_perm(pm.permissions_mask, $3)
             ))
             FROM project p WHERE p.id = $2",
        )
        .bind(user.id)
        .bind(project_id)
        .bind(Permission::ADMIN.as_i64())
        .fetch_optional(pool)
        .await
        .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?
        .unwrap_or(false);

        if !can_delete && author != Some(user.id) {
            return Err(ServerFnError::new(
                "You do not have permission to delete this task.",
            ));
        }
    } else {
        if author != Some(user.id) {
            return Err(ServerFnError::new(
                "You do not have permission to delete this task.",
            ));
        }
    }

    sqlx::query("DELETE FROM task WHERE id = $1")
        .bind(task_id)
        .execute(pool)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to delete task: {e}")))?;

    Ok(())
}

#[server]
pub async fn set_task_status(task_id: u64, status: TaskStatus) -> Result<(), ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;

    let task_meta: Option<(i64, Option<i64>)> = sqlx::query_as(
        "SELECT project_id, author FROM task WHERE id = $1",
    )
    .bind(task_id as i64)
    .fetch_optional(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?;

    let (project_id, author) = match task_meta {
        Some(m) => m,
        None => return Err(ServerFnError::new("Task not found.")),
    };

    if project_id != 0 {
        let can_mutate: bool = sqlx::query_scalar(
            "SELECT (p.leader_id = $1 OR EXISTS (
                SELECT 1 FROM project_members pm
                WHERE pm.project_id = p.id AND pm.user_id = $1 AND has_perm(pm.permissions_mask, $3)
             ))
             FROM project p WHERE p.id = $2",
        )
        .bind(user.id)
        .bind(project_id)
        .bind(Permission::EXECUTE.as_i64())
        .fetch_optional(pool)
        .await
        .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?
        .unwrap_or(false);

        if !can_mutate {
            return Err(ServerFnError::new(
                "You do not have permission to update task status in this project.",
            ));
        }
    } else {
        if author != Some(user.id) {
            let is_assigned: bool = sqlx::query_scalar(
                "SELECT COUNT(*) > 0 FROM assignments WHERE task_id = $1 AND assigned_to = $2",
            )
            .bind(task_id as i64)
            .bind(user.id)
            .fetch_one(pool)
            .await
            .unwrap_or(false);

            if !is_assigned {
                return Err(ServerFnError::new(
                    "You do not have permission to update this standalone task status.",
                ));
            }
        }
    }

    sqlx::query("UPDATE task SET status = $1 WHERE id = $2")
        .bind(status)
        .bind(task_id as i64)
        .execute(pool)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to update task: {e}")))?;

    Ok(())
}

#[server]
pub async fn get_task_messages(task_id: i64) -> Result<Vec<TaskMessage>, ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;

    let project_id: Option<Option<i64>> = sqlx::query_scalar(
        "SELECT project_id FROM task WHERE id = $1",
    )
    .bind(task_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?;

    if let Some(Some(pid)) = project_id {
        let can_read: bool = sqlx::query_scalar(
            "SELECT (p.leader_id = $1 OR EXISTS (
                SELECT 1 FROM project_members pm
                WHERE pm.project_id = p.id AND pm.user_id = $1 AND has_perm(pm.permissions_mask, $3)
             ))
             FROM project p WHERE p.id = $2",
        )
        .bind(user.id)
        .bind(pid)
        .bind(Permission::READ.as_i64())
        .fetch_optional(pool)
        .await
        .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?
        .unwrap_or(false);

        if !can_read {
            return Err(ServerFnError::new(
                "You do not have permission to view this task's messages.",
            ));
        }
    }

    let messages = sqlx::query_as::<_, TaskMessage>(
        "SELECT tm.task_id, tm.msg_id, tm.author_id,
                COALESCE(u.name, 'Deleted User') AS author_name,
                tm.created_at, tm.msg
         FROM task_message tm
         LEFT JOIN \"user\" u ON tm.author_id = u.id
         WHERE tm.task_id = $1
         ORDER BY tm.msg_id ASC",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to fetch task messages: {e}")))?;

    Ok(messages)
}

#[server]
pub async fn post_task_message(task_id: i64, msg: String) -> Result<TaskMessage, ServerFnError> {
    let text = msg.trim();
    if text.is_empty() {
        return Err(ServerFnError::new("Message cannot be empty."));
    }
    let user = get_auth_user().await?;
    let pool = get_db().await;

    let res = sqlx::query_as::<_, TaskMessage>(
        "INSERT INTO task_message (task_id, msg_id, author_id, created_at, msg)
         VALUES (
            $1,
            COALESCE((SELECT MAX(msg_id) FROM task_message WHERE task_id = $1), 0) + 1,
            $2,
            NOW(),
            $3
         )
         RETURNING task_id, msg_id, author_id, $4::TEXT AS author_name, created_at, msg",
    )
    .bind(task_id)
    .bind(user.id)
    .bind(text)
    .bind(user.name)
    .fetch_one(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to post message: {e}")))?;

    Ok(res)
}
