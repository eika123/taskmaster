use crate::types::{
    BadgeVariant, BoardSnapshot, ProjectWorkflowTransitionDto, StatusCategory, Task, TaskMessage,
    WorkflowStatusDto, WorkflowTemplate,
};
use dioxus::prelude::*;

#[cfg(feature = "server")]
use crate::types::{has_perm, Permission, ProjectViewKind, User};
#[cfg(feature = "server")]
use chrono::{DateTime, Utc};

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
    pub workflow_status_id: i64,
    pub workflow_status_name: Option<String>,
    pub start_date: Option<DateTime<Utc>>,
    pub due_date: Option<DateTime<Utc>>,
    pub estimated_hours: Option<i32>,
    pub progress_percent: Option<i32>,
}

#[server]
pub async fn suggest_task(task_title: String, description: String) -> Result<(), ServerFnError> {
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

    let initial_status_id: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM project_workflow_status WHERE project_id = $1 AND is_initial = TRUE LIMIT 1",
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);

    sqlx::query(
        "INSERT INTO task (project_id, title, description, author, workflow_status_id) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(project_id)
    .bind(task_title.trim())
    .bind(description.trim())
    .bind(user.id)
    .bind(initial_status_id)
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

    sqlx::query(
        "INSERT INTO assignments (task_id, assigned_to) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
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
pub async fn get_my_tasks(after_id: Option<i64>, amount: u64) -> Result<Vec<Task>, ServerFnError> {
    let user = get_auth_user().await?;
    let last_id = after_id.unwrap_or(0);
    let pool = get_db().await;

    let raw_tasks = sqlx::query_as::<_, RawTask>(
        "SELECT DISTINCT task.id, task.project_id, COALESCE(project.name, 'Standalone') AS project_name,
                task.title, task.author,
                COALESCE(u_author.name, 'Deleted User') AS author_name, 
                task.description,
                task.workflow_status_id, pws.name AS workflow_status_name,
                task.start_date, task.due_date,
                task.estimated_hours, task.progress_percent
         FROM task 
         LEFT JOIN project ON task.project_id = project.id
         LEFT JOIN \"user\" u_author ON task.author = u_author.id 
         LEFT JOIN project_workflow_status pws ON task.workflow_status_id = pws.id
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
        task_ids
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(",")
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
                author_name: raw
                    .author_name
                    .unwrap_or_else(|| "Deleted User".to_string()),
                description: raw.description.unwrap_or_default(),
                workflow_status_id: raw.workflow_status_id,
                workflow_status_name: raw.workflow_status_name,
                start_date: raw.start_date,
                due_date: raw.due_date,
                estimated_hours: raw.estimated_hours,
                progress_percent: raw.progress_percent.unwrap_or(0),
                assignees,
            }
        })
        .collect();

    Ok(tasks)
}

#[server]
pub async fn get_all_tasks(after_id: Option<i64>, amount: u64) -> Result<Vec<Task>, ServerFnError> {
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
                task.description,
                task.workflow_status_id, pws.name AS workflow_status_name,
                task.start_date, task.due_date,
                task.estimated_hours, task.progress_percent
         FROM task 
         LEFT JOIN project ON task.project_id = project.id
         LEFT JOIN \"user\" u ON task.author = u.id 
         LEFT JOIN project_workflow_status pws ON task.workflow_status_id = pws.id
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
        task_ids
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(",")
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
                author_name: raw
                    .author_name
                    .unwrap_or_else(|| "Deleted User".to_string()),
                description: raw.description.unwrap_or_default(),
                workflow_status_id: raw.workflow_status_id,
                workflow_status_name: raw.workflow_status_name,
                start_date: raw.start_date,
                due_date: raw.due_date,
                estimated_hours: raw.estimated_hours,
                progress_percent: raw.progress_percent.unwrap_or(0),
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
                task.description,
                task.workflow_status_id, pws.name AS workflow_status_name,
                task.start_date, task.due_date,
                task.estimated_hours, task.progress_percent
         FROM task 
         LEFT JOIN project ON task.project_id = project.id
         LEFT JOIN \"user\" u ON task.author = u.id 
         LEFT JOIN project_workflow_status pws ON task.workflow_status_id = pws.id
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
        author_name: raw
            .author_name
            .unwrap_or_else(|| "Deleted User".to_string()),
        description: raw.description.unwrap_or_default(),
        workflow_status_id: raw.workflow_status_id,
        workflow_status_name: raw.workflow_status_name,
        start_date: raw.start_date,
        due_date: raw.due_date,
        estimated_hours: raw.estimated_hours,
        progress_percent: raw.progress_percent.unwrap_or(0),
        assignees,
    })
}

#[server]
pub async fn update_task(
    task_id: i64,
    title: String,
    description: String,
    workflow_status_id: i64,
    assignee_ids: Vec<i64>,
) -> Result<Task, ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ServerFnError::new(format!("Transaction failed: {e}")))?;

    let task_meta: Option<(i64, Option<i64>)> =
        sqlx::query_as("SELECT project_id, author FROM task WHERE id = $1")
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

    // Check status semantic flags
    let target_status: Option<(bool, bool)> = sqlx::query_as(
        "SELECT allows_assignment, requires_assignee FROM project_workflow_status WHERE id = $1 AND project_id = $2",
    )
    .bind(workflow_status_id)
    .bind(project_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?;

    if let Some((allows_assignment, requires_assignee)) = target_status {
        if !allows_assignment && !assignee_ids.is_empty() {
            return Err(ServerFnError::new(
                "Cannot assign users to a task in a status that disallows assignment.",
            ));
        }
        if requires_assignee && assignee_ids.is_empty() {
            return Err(ServerFnError::new(
                "Cannot set status to one requiring an assignee without at least one assignee.",
            ));
        }
    }

    // Update title and description first
    sqlx::query("UPDATE task SET title = $1, description = $2 WHERE id = $3")
        .bind(title.trim())
        .bind(description.trim())
        .bind(task_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to update task details: {e}")))?;

    let target_requires_assignee = target_status.map(|(_, req)| req).unwrap_or(false);

    if target_requires_assignee {
        // Stage task to a status that allows assignments and does not require an assignee
        let staging_status_id: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM project_workflow_status
             WHERE project_id = $1 AND allows_assignment = TRUE AND requires_assignee = FALSE
             ORDER BY position ASC LIMIT 1",
        )
        .bind(project_id)
        .fetch_optional(&mut *tx)
        .await
        .unwrap_or(None);

        if let Some(stage_id) = staging_status_id {
            sqlx::query("UPDATE task SET workflow_status_id = $1 WHERE id = $2")
                .bind(stage_id)
                .bind(task_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| ServerFnError::new(format!("Failed to stage task status: {e}")))?;
        }

        // Insert new assignees
        for user_id in &assignee_ids {
            sqlx::query(
                "INSERT INTO assignments (task_id, assigned_to) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            )
            .bind(task_id)
            .bind(*user_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| ServerFnError::new(format!("Failed to add assignee: {e}")))?;
        }

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

        // Now set final status
        sqlx::query("UPDATE task SET workflow_status_id = $1 WHERE id = $2")
            .bind(workflow_status_id)
            .bind(task_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| ServerFnError::new(format!("Failed to set final workflow status: {e}")))?;
    } else {
        // Target does not require assignee, so update status first
        sqlx::query("UPDATE task SET workflow_status_id = $1 WHERE id = $2")
            .bind(workflow_status_id)
            .bind(task_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| ServerFnError::new(format!("Failed to update workflow status: {e}")))?;

        for user_id in &assignee_ids {
            sqlx::query(
                "INSERT INTO assignments (task_id, assigned_to) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            )
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
    }

    tx.commit()
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to commit task update: {e}")))?;

    get_task(task_id).await
}

#[server]
pub async fn delete_task(task_id: i64) -> Result<(), ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;

    let task_meta: Option<(i64, Option<i64>)> =
        sqlx::query_as("SELECT project_id, author FROM task WHERE id = $1")
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
                "You do not have permission to delete this standalone task.",
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
pub async fn set_task_status(task_id: u64, new_workflow_status_id: i64) -> Result<(), ServerFnError> {
    move_task_workflow_status(task_id as i64, new_workflow_status_id).await?;
    Ok(())
}

#[server]
pub async fn move_task_workflow_status(
    task_id: i64,
    new_workflow_status_id: i64,
) -> Result<Task, ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;

    let task_meta: Option<(i64, Option<i64>, Option<i64>)> =
        sqlx::query_as("SELECT project_id, author, workflow_status_id FROM task WHERE id = $1")
            .bind(task_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?;

    let (project_id, _author, current_ws_id) = match task_meta {
        Some(m) => m,
        None => return Err(ServerFnError::new("Task not found.")),
    };

    if project_id == 0 {
        return Err(ServerFnError::new(
            "Standalone tasks do not belong to a project workflow.",
        ));
    }

    let target_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM project_workflow_status WHERE id = $1 AND project_id = $2)",
    )
    .bind(new_workflow_status_id)
    .bind(project_id)
    .fetch_one(pool)
    .await
    .unwrap_or(false);

    if !target_exists {
        return Err(ServerFnError::new(
            "Target workflow status does not belong to this project.",
        ));
    }

    let member_perm: Option<i64> = sqlx::query_scalar(
        "SELECT CASE
            WHEN p.leader_id = $1 THEN $3
            ELSE pm.permissions_mask
         END
         FROM project p
         LEFT JOIN project_members pm ON pm.project_id = p.id AND pm.user_id = $1
         WHERE p.id = $2",
    )
    .bind(user.id)
    .bind(project_id)
    .bind(Permission::ADMIN_DEFAULT.as_i64())
    .fetch_optional(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?
    .flatten();

    let effective_mask = match member_perm {
        Some(mask) if has_perm(mask, Permission::EXECUTE) => mask,
        _ => {
            return Err(ServerFnError::new(
                "You do not have permission to update task status in this project.",
            ));
        }
    };

    let has_transitions: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM project_workflow_transition WHERE project_id = $1)",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await
    .unwrap_or(false);

    if has_transitions {
        if let Some(from_id) = current_ws_id {
            if from_id != new_workflow_status_id {
                let required_perm: Option<i64> = sqlx::query_scalar(
                    "SELECT required_permission FROM project_workflow_transition
                     WHERE project_id = $1 AND from_status_id = $2 AND to_status_id = $3",
                )
                .bind(project_id)
                .bind(from_id)
                .bind(new_workflow_status_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?;

                match required_perm {
                    Some(req) => {
                        if (effective_mask & req) == 0 {
                            return Err(ServerFnError::new(
                                "You do not have permission to perform this workflow transition.",
                            ));
                        }
                    }
                    None => {
                        return Err(ServerFnError::new(
                            "This workflow transition is not permitted for this project.",
                        ));
                    }
                }
            }
        }
    }

    sqlx::query("UPDATE task SET workflow_status_id = $1 WHERE id = $2")
        .bind(new_workflow_status_id)
        .bind(task_id)
        .execute(pool)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to move task: {e}")))?;

    get_task(task_id).await
}

#[server]
pub async fn get_project_workflow_statuses(
    project_id: i64,
) -> Result<Vec<WorkflowStatusDto>, ServerFnError> {
    let user = get_auth_user().await?;
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
            "You do not have permission to view this project.",
        ));
    }

    let statuses = sqlx::query_as::<_, WorkflowStatusDto>(
        "SELECT pws.id, pws.project_id, pws.name, pws.category, pws.badge_variant,
                pws.position, pws.wip_limit, pws.is_initial, pws.allows_assignment,
                pws.requires_assignee,
                COUNT(t.id)::bigint AS task_count,
                (pws.wip_limit IS NOT NULL AND COUNT(t.id) > pws.wip_limit) AS wip_exceeded,
                pws.created_at
         FROM project_workflow_status pws
         LEFT JOIN task t ON t.project_id = pws.project_id AND t.workflow_status_id = pws.id
         WHERE pws.project_id = $1
         GROUP BY pws.id
         ORDER BY pws.position ASC, pws.id ASC",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to fetch workflow statuses: {e}")))?;

    Ok(statuses)
}

#[server]
pub async fn get_project_workflow_transitions(
    project_id: i64,
) -> Result<Vec<ProjectWorkflowTransitionDto>, ServerFnError> {
    let user = get_auth_user().await?;
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
            "You do not have permission to view this project.",
        ));
    }

    let transitions = sqlx::query_as::<_, ProjectWorkflowTransitionDto>(
        "SELECT project_id, from_status_id, to_status_id, required_permission
         FROM project_workflow_transition
         WHERE project_id = $1
         ORDER BY from_status_id ASC, to_status_id ASC",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to fetch workflow transitions: {e}")))?;

    Ok(transitions)
}

#[server]
pub async fn get_project_board_snapshot(project_id: i64) -> Result<BoardSnapshot, ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;

    let project_row: Option<(String, i64, Option<i64>)> = sqlx::query_as(
        "SELECT p.name,
                p.leader_id,
                pm.permissions_mask
         FROM project p
         LEFT JOIN project_members pm ON pm.project_id = p.id AND pm.user_id = $1
         WHERE p.id = $2",
    )
    .bind(user.id)
    .bind(project_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?;

    let (project_name, leader_id, member_mask) = match project_row {
        Some(r) => r,
        None => return Err(ServerFnError::new("Project not found.")),
    };

    let effective_mask = if leader_id == user.id {
        Permission::ADMIN_DEFAULT.as_i64()
    } else {
        member_mask.unwrap_or(0)
    };

    if !has_perm(effective_mask, Permission::READ) {
        return Err(ServerFnError::new(
            "You do not have permission to view this project.",
        ));
    }

    let statuses = sqlx::query_as::<_, WorkflowStatusDto>(
        "SELECT pws.id, pws.project_id, pws.name, pws.category, pws.badge_variant,
                pws.position, pws.wip_limit, pws.is_initial, pws.allows_assignment,
                pws.requires_assignee,
                COUNT(t.id)::bigint AS task_count,
                (pws.wip_limit IS NOT NULL AND COUNT(t.id) > pws.wip_limit) AS wip_exceeded,
                pws.created_at
         FROM project_workflow_status pws
         LEFT JOIN task t ON t.project_id = pws.project_id AND t.workflow_status_id = pws.id
         WHERE pws.project_id = $1
         GROUP BY pws.id
         ORDER BY pws.position ASC, pws.id ASC",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to fetch workflow statuses: {e}")))?;

    let tasks = get_project_tasks(project_id, None, 1000).await?;

    Ok(BoardSnapshot {
        project_id,
        project_name,
        enabled_views: vec![
            ProjectViewKind::KanbanClassic,
            ProjectViewKind::KanbanSwimlanes,
            ProjectViewKind::List,
            ProjectViewKind::Gantt,
        ],
        default_view: ProjectViewKind::KanbanClassic,
        caller_permissions: effective_mask,
        statuses,
        tasks,
        dependencies: Vec::new(),
    })
}

#[server]
pub async fn get_task_messages(task_id: i64) -> Result<Vec<TaskMessage>, ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;

    let project_id: Option<Option<i64>> =
        sqlx::query_scalar("SELECT project_id FROM task WHERE id = $1")
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

#[server]
pub async fn get_workflow_templates() -> Result<Vec<WorkflowTemplate>, ServerFnError> {
    Ok(WorkflowTemplate::starter_templates())
}

#[server]
pub async fn apply_workflow_template(
    project_id: i64,
    template_id: String,
) -> Result<Vec<WorkflowStatusDto>, ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;

    let can_manage: bool = sqlx::query_scalar(
        "SELECT (p.leader_id = $1 OR EXISTS (
            SELECT 1 FROM project_members pm
            WHERE pm.project_id = p.id AND pm.user_id = $1 AND has_perm(pm.permissions_mask, $3)
         ))
         FROM project p WHERE p.id = $2",
    )
    .bind(user.id)
    .bind(project_id)
    .bind(Permission::MANAGE_WORKFLOW.as_i64())
    .fetch_optional(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?
    .unwrap_or(false);

    if !can_manage {
        return Err(ServerFnError::new(
            "You do not have permission to manage the workflow for this project.",
        ));
    }

    let templates = WorkflowTemplate::starter_templates();
    let tpl = match templates.into_iter().find(|t| {
        t.id == template_id
            || t.id.starts_with(&template_id)
            || template_id.starts_with(&t.id)
            || (template_id == "scrum" && t.id == "software_sprint")
            || (template_id == "kanban" && t.id == "classic_kanban")
            || (template_id == "guild_raid" && t.id == "guild_raid_prep")
            || (template_id == "legacy" && t.id == "legacy_flow")
    }) {
        Some(t) => t,
        None => return Err(ServerFnError::new("Template not found.")),
    };

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ServerFnError::new(format!("Transaction failed: {e}")))?;

    // Temporarily unset is_initial and rename existing statuses to avoid unique constraint collisions
    sqlx::query(
        "UPDATE project_workflow_status
         SET is_initial = FALSE, name = '__old_' || id || '_' || name
         WHERE project_id = $1",
    )
    .bind(project_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to prep existing statuses: {e}")))?;

    let mut new_initial_id: Option<i64> = None;
    let mut new_status_ids: Vec<i64> = Vec::new();

    for s in tpl.statuses {
        let inserted_id: i64 = sqlx::query_scalar(
            "INSERT INTO project_workflow_status (
                project_id, name, category, badge_variant, position, wip_limit,
                is_initial, allows_assignment, requires_assignee
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING id",
        )
        .bind(project_id)
        .bind(&s.name)
        .bind(s.category.as_str())
        .bind(s.badge_variant.as_str())
        .bind(s.position)
        .bind(s.wip_limit)
        .bind(s.is_initial)
        .bind(s.allows_assignment)
        .bind(s.requires_assignee)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to insert status: {e}")))?;

        new_status_ids.push(inserted_id);
        if s.is_initial {
            new_initial_id = Some(inserted_id);
        }
    }

    let initial_status_id = new_initial_id.unwrap_or_else(|| new_status_ids[0]);

    sqlx::query("UPDATE task SET workflow_status_id = $1 WHERE project_id = $2 AND workflow_status_id NOT IN (SELECT UNNEST($3::bigint[]))")
        .bind(initial_status_id)
        .bind(project_id)
        .bind(&new_status_ids)
        .execute(&mut *tx)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to remap tasks to new initial status: {e}")))?;

    sqlx::query("DELETE FROM project_workflow_transition WHERE project_id = $1")
        .bind(project_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to clear transitions: {e}")))?;

    sqlx::query("DELETE FROM project_workflow_status WHERE project_id = $1 AND id NOT IN (SELECT UNNEST($2::bigint[]))")
        .bind(project_id)
        .bind(&new_status_ids)
        .execute(&mut *tx)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to delete old statuses: {e}")))?;

    tx.commit()
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to commit template application: {e}")))?;

    get_project_workflow_statuses(project_id).await
}

#[server]
pub async fn create_workflow_status(
    project_id: i64,
    name: String,
    category: StatusCategory,
    badge_variant: BadgeVariant,
    wip_limit: Option<i32>,
    allows_assignment: bool,
    requires_assignee: bool,
) -> Result<WorkflowStatusDto, ServerFnError> {
    let name_clean = name.trim();
    if name_clean.is_empty() {
        return Err(ServerFnError::new("Status name cannot be empty."));
    }

    let user = get_auth_user().await?;
    let pool = get_db().await;

    let can_manage: bool = sqlx::query_scalar(
        "SELECT (p.leader_id = $1 OR EXISTS (
            SELECT 1 FROM project_members pm
            WHERE pm.project_id = p.id AND pm.user_id = $1 AND has_perm(pm.permissions_mask, $3)
         ))
         FROM project p WHERE p.id = $2",
    )
    .bind(user.id)
    .bind(project_id)
    .bind(Permission::MANAGE_WORKFLOW.as_i64())
    .fetch_optional(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?
    .unwrap_or(false);

    if !can_manage {
        return Err(ServerFnError::new(
            "You do not have permission to manage the workflow for this project.",
        ));
    }

    let max_pos: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(position), 0) FROM project_workflow_status WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let next_pos = max_pos + 1000;

    let status = sqlx::query_as::<_, WorkflowStatusDto>(
        "INSERT INTO project_workflow_status (
            project_id, name, category, badge_variant, position, wip_limit,
            is_initial, allows_assignment, requires_assignee
        ) VALUES ($1, $2, $3, $4, $5, $6, FALSE, $7, $8)
        RETURNING id, project_id, name, category, badge_variant, position, wip_limit,
                  is_initial, allows_assignment, requires_assignee,
                  0::bigint AS task_count, FALSE AS wip_exceeded, created_at",
    )
    .bind(project_id)
    .bind(name_clean)
    .bind(category.as_str())
    .bind(badge_variant.as_str())
    .bind(next_pos)
    .bind(wip_limit)
    .bind(allows_assignment)
    .bind(requires_assignee)
    .fetch_one(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to create workflow status: {e}")))?;

    Ok(status)
}

#[server]
pub async fn update_workflow_status(
    status_id: i64,
    name: String,
    category: StatusCategory,
    badge_variant: BadgeVariant,
    wip_limit: Option<i32>,
    allows_assignment: bool,
    requires_assignee: bool,
) -> Result<WorkflowStatusDto, ServerFnError> {
    let name_clean = name.trim();
    if name_clean.is_empty() {
        return Err(ServerFnError::new("Status name cannot be empty."));
    }

    let user = get_auth_user().await?;
    let pool = get_db().await;

    let project_id: i64 = match sqlx::query_scalar("SELECT project_id FROM project_workflow_status WHERE id = $1")
        .bind(status_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?
    {
        Some(pid) => pid,
        None => return Err(ServerFnError::new("Status not found.")),
    };

    let can_manage: bool = sqlx::query_scalar(
        "SELECT (p.leader_id = $1 OR EXISTS (
            SELECT 1 FROM project_members pm
            WHERE pm.project_id = p.id AND pm.user_id = $1 AND has_perm(pm.permissions_mask, $3)
         ))
         FROM project p WHERE p.id = $2",
    )
    .bind(user.id)
    .bind(project_id)
    .bind(Permission::MANAGE_WORKFLOW.as_i64())
    .fetch_optional(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?
    .unwrap_or(false);

    if !can_manage {
        return Err(ServerFnError::new(
            "You do not have permission to manage the workflow for this project.",
        ));
    }

    let status = sqlx::query_as::<_, WorkflowStatusDto>(
        "UPDATE project_workflow_status
         SET name = $1, category = $2, badge_variant = $3, wip_limit = $4,
             allows_assignment = $5, requires_assignee = $6
         WHERE id = $7
         RETURNING id, project_id, name, category, badge_variant, position, wip_limit,
                   is_initial, allows_assignment, requires_assignee,
                   (SELECT COUNT(*) FROM task WHERE workflow_status_id = $7)::bigint AS task_count,
                   (wip_limit IS NOT NULL AND (SELECT COUNT(*) FROM task WHERE workflow_status_id = $7) > wip_limit) AS wip_exceeded,
                   created_at",
    )
    .bind(name_clean)
    .bind(category.as_str())
    .bind(badge_variant.as_str())
    .bind(wip_limit)
    .bind(allows_assignment)
    .bind(requires_assignee)
    .bind(status_id)
    .fetch_one(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to update workflow status: {e}")))?;

    Ok(status)
}

#[server]
pub async fn delete_workflow_status(
    status_id: i64,
    remap_to_status_id: Option<i64>,
) -> Result<(), ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;

    let row: Option<(i64, bool)> =
        sqlx::query_as("SELECT project_id, is_initial FROM project_workflow_status WHERE id = $1")
            .bind(status_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?;

    let (project_id, is_initial) = match row {
        Some(r) => r,
        None => return Err(ServerFnError::new("Status not found.")),
    };

    let can_manage: bool = sqlx::query_scalar(
        "SELECT (p.leader_id = $1 OR EXISTS (
            SELECT 1 FROM project_members pm
            WHERE pm.project_id = p.id AND pm.user_id = $1 AND has_perm(pm.permissions_mask, $3)
         ))
         FROM project p WHERE p.id = $2",
    )
    .bind(user.id)
    .bind(project_id)
    .bind(Permission::MANAGE_WORKFLOW.as_i64())
    .fetch_optional(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?
    .unwrap_or(false);

    if !can_manage {
        return Err(ServerFnError::new(
            "You do not have permission to manage the workflow for this project.",
        ));
    }

    let status_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM project_workflow_status WHERE project_id = $1")
        .bind(project_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);

    if status_count <= 1 {
        return Err(ServerFnError::new(
            "Cannot delete the only workflow status in a project.",
        ));
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ServerFnError::new(format!("Transaction failed: {e}")))?;

    let task_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM task WHERE workflow_status_id = $1")
        .bind(status_id)
        .fetch_one(&mut *tx)
        .await
        .unwrap_or(0);

    if task_count > 0 {
        let remap_id = match remap_to_status_id {
            Some(rid) if rid != status_id => rid,
            _ => {
                return Err(ServerFnError::new(
                    "Cannot delete status: tasks exist in this status and no valid remap target was provided.",
                ));
            }
        };

        let target_valid: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM project_workflow_status WHERE id = $1 AND project_id = $2)",
        )
        .bind(remap_id)
        .bind(project_id)
        .fetch_one(&mut *tx)
        .await
        .unwrap_or(false);

        if !target_valid {
            return Err(ServerFnError::new(
                "Remap target status does not belong to this project.",
            ));
        }

        sqlx::query("UPDATE task SET workflow_status_id = $1 WHERE workflow_status_id = $2")
            .bind(remap_id)
            .bind(status_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| ServerFnError::new(format!("Failed to remap tasks: {e}")))?;
    }

    sqlx::query("DELETE FROM project_workflow_status WHERE id = $1")
        .bind(status_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to delete workflow status: {e}")))?;

    if is_initial {
        let new_initial_id: Option<i64> = match remap_to_status_id {
            Some(rid) => Some(rid),
            None => sqlx::query_scalar(
                "SELECT id FROM project_workflow_status WHERE project_id = $1 ORDER BY position ASC LIMIT 1",
            )
            .bind(project_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?,
        };

        if let Some(nid) = new_initial_id {
            sqlx::query("UPDATE project_workflow_status SET is_initial = TRUE WHERE id = $1")
                .bind(nid)
                .execute(&mut *tx)
                .await
                .map_err(|e| ServerFnError::new(format!("Failed to promote new initial status: {e}")))?;
        }
    }

    tx.commit()
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to commit status deletion: {e}")))?;

    Ok(())
}

#[server]
pub async fn reorder_workflow_statuses(
    project_id: i64,
    status_ids_in_order: Vec<i64>,
) -> Result<Vec<WorkflowStatusDto>, ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;

    let can_manage: bool = sqlx::query_scalar(
        "SELECT (p.leader_id = $1 OR EXISTS (
            SELECT 1 FROM project_members pm
            WHERE pm.project_id = p.id AND pm.user_id = $1 AND has_perm(pm.permissions_mask, $3)
         ))
         FROM project p WHERE p.id = $2",
    )
    .bind(user.id)
    .bind(project_id)
    .bind(Permission::MANAGE_WORKFLOW.as_i64())
    .fetch_optional(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?
    .unwrap_or(false);

    if !can_manage {
        return Err(ServerFnError::new(
            "You do not have permission to manage the workflow for this project.",
        ));
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ServerFnError::new(format!("Transaction failed: {e}")))?;

    for (index, sid) in status_ids_in_order.iter().enumerate() {
        let pos = ((index + 1) * 1000) as i64;
        sqlx::query("UPDATE project_workflow_status SET position = $1 WHERE id = $2 AND project_id = $3")
            .bind(pos)
            .bind(*sid)
            .bind(project_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| ServerFnError::new(format!("Failed to update position: {e}")))?;
    }

    tx.commit()
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to commit reordering: {e}")))?;

    get_project_workflow_statuses(project_id).await
}
