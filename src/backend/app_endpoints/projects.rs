use dioxus::prelude::*;
use crate::types::{Project, ProjectMember};

#[cfg(feature = "server")]
use crate::backend::app_endpoints::auth::get_auth_user;
#[cfg(feature = "server")]
use crate::backend::database::get_db;

#[server]
pub async fn get_user_projects() -> Result<Vec<Project>, ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;

    let projects = sqlx::query_as::<_, Project>(
        "SELECT p.id, p.name, p.description, p.owner_org_id, o.name AS owner_org_name,
                p.leader_id, u.name AS leader_name, p.created_at
         FROM project p
         LEFT JOIN organization o ON p.owner_org_id = o.id
         INNER JOIN user u ON p.leader_id = u.id
         WHERE p.leader_id = $1
            OR EXISTS (SELECT 1 FROM project_members pm WHERE pm.project_id = p.id AND pm.user_id = $1)
         ORDER BY p.id ASC",
    )
    .bind(user.id)
    .fetch_all(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to fetch projects: {e}")))?;

    Ok(projects)
}

#[server]
pub async fn get_project(project_id: i64) -> Result<Project, ServerFnError> {
    let pool = get_db().await;
    let proj = sqlx::query_as::<_, Project>(
        "SELECT p.id, p.name, p.description, p.owner_org_id, o.name AS owner_org_name,
                p.leader_id, u.name AS leader_name, p.created_at
         FROM project p
         LEFT JOIN organization o ON p.owner_org_id = o.id
         INNER JOIN user u ON p.leader_id = u.id
         WHERE p.id = $1",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Project not found: {e}")))?;

    Ok(proj)
}

#[server]
pub async fn create_project(
    name: String,
    description: String,
    owner_org_id: Option<i64>,
) -> Result<Project, ServerFnError> {
    let user = get_auth_user().await?;
    let name_clean = name.trim();
    if name_clean.is_empty() {
        return Err(ServerFnError::new("Project name is required."));
    }

    let pool = get_db().await;
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ServerFnError::new(format!("Transaction failed: {e}")))?;

    let res = sqlx::query(
        "INSERT INTO project (name, description, owner_org_id, leader_id) VALUES ($1, $2, $3, $4)",
    )
    .bind(name_clean)
    .bind(description.trim())
    .bind(owner_org_id)
    .bind(user.id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to create project: {e}")))?;

    let proj_id = res.last_insert_rowid();

    sqlx::query("INSERT INTO project_members (project_id, user_id, role) VALUES ($1, $2, 'admin')")
        .bind(proj_id)
        .bind(user.id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to register leader: {e}")))?;

    tx.commit()
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to commit project: {e}")))?;

    get_project(proj_id).await
}

#[server]
pub async fn delete_project(project_id: i64) -> Result<(), ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;

    let leader_id: i64 = sqlx::query_scalar("SELECT leader_id FROM project WHERE id = $1")
        .bind(project_id)
        .fetch_one(pool)
        .await
        .map_err(|e| ServerFnError::new(format!("Project not found: {e}")))?;

    if leader_id != user.id {
        return Err(ServerFnError::new(
            "Only the project leader can delete the project.",
        ));
    }

    sqlx::query("DELETE FROM project WHERE id = $1")
        .bind(project_id)
        .execute(pool)
        .await
        .map_err(|e| {
            ServerFnError::new(format!(
                "Cannot delete project: {e}. Ensure all other members are removed first."
            ))
        })?;

    Ok(())
}

#[server]
pub async fn get_project_members(project_id: i64) -> Result<Vec<ProjectMember>, ServerFnError> {
    let pool = get_db().await;
    let members = sqlx::query_as::<_, ProjectMember>(
        "SELECT pm.project_id, pm.user_id, u.name AS user_name, u.email AS user_email,
                pm.role, (pm.granted_by_org_role != 0) AS granted_by_org_role, pm.joined_at
         FROM project_members pm
         INNER JOIN user u ON pm.user_id = u.id
         WHERE pm.project_id = $1
         ORDER BY u.name ASC",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to fetch project members: {e}")))?;

    Ok(members)
}

#[server]
pub async fn invite_user_to_project(
    project_id: i64,
    invited_user_id: i64,
    role: String,
    message: String,
) -> Result<(), ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;

    sqlx::query(
        "INSERT INTO project_invitation (project_id, invited_user_id, invited_by, role, message)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT(project_id, invited_user_id) DO UPDATE SET
            role = excluded.role,
            message = excluded.message,
            created_at = unixepoch()",
    )
    .bind(project_id)
    .bind(invited_user_id)
    .bind(user.id)
    .bind(role)
    .bind(message.trim())
    .execute(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to send project invitation: {e}")))?;

    Ok(())
}

#[server]
pub async fn respond_to_project_invitation(
    project_id: i64,
    accept: bool,
) -> Result<(), ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;

    let invite: Option<(String,)> = sqlx::query_as(
        "SELECT role FROM project_invitation WHERE project_id = $1 AND invited_user_id = $2",
    )
    .bind(project_id)
    .bind(user.id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?;

    let (role,) = match invite {
        Some(inv) => inv,
        None => return Err(ServerFnError::new("Invitation not found.")),
    };

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ServerFnError::new(format!("Transaction error: {e}")))?;

    sqlx::query("DELETE FROM project_invitation WHERE project_id = $1 AND invited_user_id = $2")
        .bind(project_id)
        .bind(user.id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to clear invite: {e}")))?;

    if accept {
        sqlx::query(
            "INSERT INTO project_members (project_id, user_id, role)
             VALUES ($1, $2, $3)
             ON CONFLICT(project_id, user_id) DO UPDATE SET role = excluded.role",
        )
        .bind(project_id)
        .bind(user.id)
        .bind(role)
        .execute(&mut *tx)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to join project: {e}")))?;
    }

    tx.commit()
        .await
        .map_err(|e| ServerFnError::new(format!("Commit failed: {e}")))?;

    Ok(())
}

#[server]
pub async fn request_project_leadership_transfer(
    project_id: i64,
    requested_new_leader: i64,
) -> Result<(), ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;

    let leader_id: i64 = sqlx::query_scalar("SELECT leader_id FROM project WHERE id = $1")
        .bind(project_id)
        .fetch_one(pool)
        .await
        .map_err(|e| ServerFnError::new(format!("Project not found: {e}")))?;

    if leader_id != user.id {
        return Err(ServerFnError::new("Only current leader can initiate transfer."));
    }

    sqlx::query(
        "INSERT INTO project_leadership_transfer (project_id, requested_new_leader, created_at)
         VALUES ($1, $2, unixepoch())
         ON CONFLICT(project_id) DO UPDATE SET
            requested_new_leader = excluded.requested_new_leader,
            created_at = unixepoch()",
    )
    .bind(project_id)
    .bind(requested_new_leader)
    .execute(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to request leadership transfer: {e}")))?;

    Ok(())
}

#[server]
pub async fn respond_project_leadership_transfer(
    project_id: i64,
    accept: bool,
) -> Result<(), ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;

    let target: Option<(i64,)> = sqlx::query_as(
        "SELECT requested_new_leader FROM project_leadership_transfer WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?;

    if target.map(|(t,)| t) != Some(user.id) {
        return Err(ServerFnError::new("No pending leadership transfer for you."));
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ServerFnError::new(format!("Transaction error: {e}")))?;

    sqlx::query("DELETE FROM project_leadership_transfer WHERE project_id = $1")
        .bind(project_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to clear transfer request: {e}")))?;

    if accept {
        sqlx::query("UPDATE project SET leader_id = $1 WHERE id = $2")
            .bind(user.id)
            .bind(project_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| ServerFnError::new(format!("Failed to transfer leadership: {e}")))?;
    }

    tx.commit()
        .await
        .map_err(|e| ServerFnError::new(format!("Commit failed: {e}")))?;

    Ok(())
}

