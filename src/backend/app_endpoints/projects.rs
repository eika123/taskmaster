use dioxus::prelude::*;
use crate::types::{Project, ProjectMember, ProjectMemberGrant, ProjectOrganization, ProjectOrgRoleRule};

#[cfg(feature = "server")]
use crate::backend::app_endpoints::auth::get_auth_user;
#[cfg(feature = "server")]
use crate::backend::database::get_db;

#[server]
pub async fn get_user_projects() -> Result<Vec<Project>, ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;

    let projects = sqlx::query_as::<_, Project>(
        "SELECT p.id, p.name, p.description, p.owner_org_id, org.name AS owner_org_name,
                p.leader_id, u.name AS leader_name, p.created_at
         FROM project p
         LEFT JOIN organization org ON p.owner_org_id = org.id
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
        "SELECT p.id, p.name, p.description, p.owner_org_id, org.name AS owner_org_name,
                p.leader_id, u.name AS leader_name, p.created_at
         FROM project p
         LEFT JOIN organization org ON p.owner_org_id = org.id
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

    sqlx::query("INSERT INTO project_members (project_id, user_id, role, permissions_mask) VALUES ($1, $2, 'admin', 63)")
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
pub async fn delete_project(project_id: i64, confirmation: String) -> Result<(), ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;

    let (leader_id, name): (i64, String) =
        sqlx::query_as("SELECT leader_id, name FROM project WHERE id = $1")
            .bind(project_id)
            .fetch_one(pool)
            .await
            .map_err(|e| ServerFnError::new(format!("Project not found: {e}")))?;

    if leader_id != user.id {
        return Err(ServerFnError::new(
            "Only the project leader can delete the project.",
        ));
    }

    let expected = format!("DELETE {}", name.trim());
    if confirmation.trim() != expected {
        return Err(ServerFnError::new(format!(
            "Confirmation mismatch. Please type exactly '{expected}' to delete this project."
        )));
    }

    // SQLite's ON DELETE CASCADE automatically cascades deletion to tasks,
    // assignments, messages, and member associations. The database trigger
    // `prevent_delete_populated_project` guarantees that projects with other members
    // cannot be deleted.
    sqlx::query("DELETE FROM project WHERE id = $1")
        .bind(project_id)
        .execute(pool)
        .await
        .map_err(|e| {
            ServerFnError::new(format!("Cannot delete project: {e}."))
        })?;

    Ok(())
}

#[server]
pub async fn remove_project_member(
    project_id: i64,
    member_user_id: i64,
) -> Result<(), ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;

    let leader_id: i64 = sqlx::query_scalar("SELECT leader_id FROM project WHERE id = $1")
        .bind(project_id)
        .fetch_one(pool)
        .await
        .map_err(|e| ServerFnError::new(format!("Project not found: {e}")))?;

    if user.id != member_user_id && user.id != leader_id {
        return Err(ServerFnError::new(
            "Only the project leader can remove other members.",
        ));
    }

    // The database trigger `prevent_remove_project_leader` will automatically block
    // the project leader from removing themselves unless leadership is transferred first!
    sqlx::query("DELETE FROM project_members WHERE project_id = $1 AND user_id = $2")
        .bind(project_id)
        .bind(member_user_id)
        .execute(pool)
        .await
        .map_err(|e| ServerFnError::new(format!("Cannot remove member: {e}")))?;

    Ok(())
}

#[server]
pub async fn get_project_members(project_id: i64) -> Result<Vec<ProjectMember>, ServerFnError> {
    let pool = get_db().await;
    let members = sqlx::query_as::<_, ProjectMember>(
        "SELECT pm.project_id, pm.user_id, u.name AS user_name, u.email AS user_email,
                pm.role, (pm.granted_by_org_role != 0) AS granted_by_org_role,
                pm.permissions_mask, pm.active_grant_key, pm.joined_at
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

#[server]
pub async fn affiliate_organization_with_project(
    project_id: i64,
    organization_id: i64,
    relationship_type: String,
    max_granted_role: String,
) -> Result<(), ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;

    // Must be project leader or project admin (permissions_mask & 32 != 0)
    let is_admin: bool = sqlx::query_scalar(
        "SELECT (permissions_mask & 32) != 0 FROM project_members WHERE project_id = $1 AND user_id = $2",
    )
    .bind(project_id)
    .bind(user.id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?
    .unwrap_or(false);

    if !is_admin {
        return Err(ServerFnError::new("Only project admins can affiliate organizations."));
    }

    let rel = relationship_type.trim().to_lowercase();
    if !["owner", "partner", "contractor", "auditor"].contains(&rel.as_str()) {
        return Err(ServerFnError::new("Invalid relationship type."));
    }

    let ceiling = max_granted_role.trim().to_lowercase();
    if !["viewer", "member", "admin"].contains(&ceiling.as_str()) {
        return Err(ServerFnError::new("Invalid max granted role ceiling."));
    }

    sqlx::query(
        "INSERT INTO project_organizations (project_id, organization_id, relationship_type, max_granted_role)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT(project_id, organization_id) DO UPDATE SET
            relationship_type = excluded.relationship_type,
            max_granted_role = excluded.max_granted_role",
    )
    .bind(project_id)
    .bind(organization_id)
    .bind(rel)
    .bind(ceiling)
    .execute(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to affiliate organization: {e}")))?;

    Ok(())
}

#[server]
pub async fn remove_organization_from_project(
    project_id: i64,
    organization_id: i64,
    confirm_active_tasks_warning: bool,
) -> Result<(), ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;

    let leader_id: Option<i64> = sqlx::query_scalar("SELECT leader_id FROM project WHERE id = $1")
        .bind(project_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?;

    let gov_id: Option<i64> = sqlx::query_scalar("SELECT governing_user_id FROM organization WHERE id = $1")
        .bind(organization_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?;

    if leader_id != Some(user.id) && gov_id != Some(user.id) {
        return Err(ServerFnError::new(
            "Only the project leader or organization governor can disaffiliate the organization.",
        ));
    }

    // Pre-flight check: will removing this org revoke task-execution access for users with active assigned tasks?
    #[derive(sqlx::FromRow)]
    struct ImpactRow {
        task_id: i64,
        task_title: String,
        status: String,
        user_name: String,
    }

    let impacts: Vec<ImpactRow> = sqlx::query_as(
        "SELECT DISTINCT t.id AS task_id, t.title AS task_title, t.status, u.name AS user_name
         FROM project_member_grants pmg
         JOIN user u ON pmg.user_id = u.id
         JOIN assignments a ON a.assigned_to = u.id
         JOIN task t ON t.id = a.task_id AND t.project_id = $1
         WHERE pmg.project_id = $1
           AND pmg.source_org_id = $2
           AND t.status != 'finished'
           AND NOT EXISTS (
               SELECT 1 FROM project_member_grants other
               JOIN project_role_definition prd ON prd.role = other.project_role
               WHERE other.project_id = $1
                 AND other.user_id = u.id
                 AND (other.source_org_id IS NULL OR other.source_org_id != $2)
                 AND (prd.permissions_mask & 4) != 0
           )",
    )
    .bind(project_id)
    .bind(organization_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Impact check failed: {e}")))?;

    if !impacts.is_empty() && !confirm_active_tasks_warning {
        let details: Vec<String> = impacts
            .iter()
            .map(|r| format!("{} has task '{}' [{}]", r.user_name, r.task_title, r.status))
            .collect();
        return Err(ServerFnError::new(format!(
            "Active task impact warning: Removing this organization will revoke task privileges for assigned users: {}. Confirm with confirm_active_tasks_warning=true or hand over tasks first.",
            details.join("; ")
        )));
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ServerFnError::new(format!("Transaction failed: {e}")))?;

    // If confirmed, unassign non-in_progress tasks
    for imp in &impacts {
        if imp.status != "in_progress" {
            sqlx::query("DELETE FROM assignments WHERE task_id = $1 AND assigned_to = (SELECT id FROM user WHERE name = $2)")
                .bind(imp.task_id)
                .bind(&imp.user_name)
                .execute(&mut *tx)
                .await
                .map_err(|e| ServerFnError::new(format!("Failed to unassign task: {e}")))?;
        }
    }

    // Removing from project_organizations cascades to project_org_role_access -> project_member_grants -> project_members
    // Database trigger prevent_remove_project_leader will abort if an affected user is project.leader_id!
    sqlx::query("DELETE FROM project_organizations WHERE project_id = $1 AND organization_id = $2")
        .bind(project_id)
        .bind(organization_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to remove organization: {e}")))?;

    tx.commit()
        .await
        .map_err(|e| ServerFnError::new(format!("Commit failed: {e}")))?;

    Ok(())
}

#[server]
pub async fn get_project_organizations(
    project_id: i64,
) -> Result<Vec<ProjectOrganization>, ServerFnError> {
    let pool = get_db().await;
    let orgs = sqlx::query_as::<_, ProjectOrganization>(
        "SELECT po.project_id, po.organization_id, o.name AS organization_name,
                po.relationship_type, po.max_granted_role, po.joined_at
         FROM project_organizations po
         INNER JOIN organization o ON po.organization_id = o.id
         WHERE po.project_id = $1
         ORDER BY o.name ASC",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to fetch project organizations: {e}")))?;

    Ok(orgs)
}

#[server]
pub async fn set_org_role_project_access(
    project_id: i64,
    organization_id: i64,
    org_role: String,
    project_role: String,
) -> Result<(), ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;

    // Caller must be org governor or org admin
    let is_org_admin: bool = sqlx::query_scalar(
        "SELECT (o.governing_user_id = $1 OR EXISTS (
            SELECT 1 FROM organization_member_roles omr
            WHERE omr.organization_id = o.id AND omr.user_id = $1 AND omr.role = 'admin'
         ))
         FROM organization o WHERE o.id = $2",
    )
    .bind(user.id)
    .bind(organization_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?
    .unwrap_or(false);

    if !is_org_admin {
        return Err(ServerFnError::new("Only organization admins can configure project role mappings."));
    }

    let p_role = project_role.trim().to_lowercase();
    let o_role = org_role.trim().to_string();

    // The database trigger check_org_project_role_ceiling_insert will check against max_granted_role!
    sqlx::query(
        "INSERT INTO project_org_role_access (project_id, organization_id, org_role, project_role)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT(project_id, organization_id, org_role) DO UPDATE SET
            project_role = excluded.project_role",
    )
    .bind(project_id)
    .bind(organization_id)
    .bind(o_role)
    .bind(p_role)
    .execute(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to set role access rule: {e}")))?;

    Ok(())
}

#[server]
pub async fn revoke_org_role_project_access(
    project_id: i64,
    organization_id: i64,
    org_role: String,
    confirm_active_tasks_warning: bool,
) -> Result<(), ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;

    let is_authorized: bool = sqlx::query_scalar(
        "SELECT (p.leader_id = $1 OR o.governing_user_id = $1 OR EXISTS (
            SELECT 1 FROM organization_member_roles omr
            WHERE omr.organization_id = o.id AND omr.user_id = $1 AND omr.role = 'admin'
         ))
         FROM project p, organization o
         WHERE p.id = $2 AND o.id = $3",
    )
    .bind(user.id)
    .bind(project_id)
    .bind(organization_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?
    .unwrap_or(false);

    if !is_authorized {
        return Err(ServerFnError::new("Unauthorized to revoke role mapping rule."));
    }

    #[derive(sqlx::FromRow)]
    struct ImpactRow {
        task_id: i64,
        task_title: String,
        status: String,
        user_name: String,
    }

    let impacts: Vec<ImpactRow> = sqlx::query_as(
        "SELECT DISTINCT t.id AS task_id, t.title AS task_title, t.status, u.name AS user_name
         FROM project_member_grants pmg
         JOIN user u ON pmg.user_id = u.id
         JOIN assignments a ON a.assigned_to = u.id
         JOIN task t ON t.id = a.task_id AND t.project_id = $1
         WHERE pmg.project_id = $1
           AND pmg.source_org_id = $2
           AND pmg.source_org_role = $3
           AND t.status != 'finished'
           AND NOT EXISTS (
               SELECT 1 FROM project_member_grants other
               JOIN project_role_definition prd ON prd.role = other.project_role
               WHERE other.project_id = $1
                 AND other.user_id = u.id
                 AND NOT (other.source_org_id = $2 AND other.source_org_role = $3)
                 AND (prd.permissions_mask & 4) != 0
           )",
    )
    .bind(project_id)
    .bind(organization_id)
    .bind(&org_role)
    .fetch_all(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Impact check failed: {e}")))?;

    if !impacts.is_empty() && !confirm_active_tasks_warning {
        let details: Vec<String> = impacts
            .iter()
            .map(|r| format!("{} has task '{}' [{}]", r.user_name, r.task_title, r.status))
            .collect();
        return Err(ServerFnError::new(format!(
            "Active task impact warning: Revoking rule will revoke task privileges for assigned users: {}. Confirm with confirm_active_tasks_warning=true or hand over tasks first.",
            details.join("; ")
        )));
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ServerFnError::new(format!("Transaction failed: {e}")))?;

    for imp in &impacts {
        if imp.status != "in_progress" {
            sqlx::query("DELETE FROM assignments WHERE task_id = $1 AND assigned_to = (SELECT id FROM user WHERE name = $2)")
                .bind(imp.task_id)
                .bind(&imp.user_name)
                .execute(&mut *tx)
                .await
                .map_err(|e| ServerFnError::new(format!("Failed to unassign task: {e}")))?;
        }
    }

    sqlx::query("DELETE FROM project_org_role_access WHERE project_id = $1 AND organization_id = $2 AND org_role = $3")
        .bind(project_id)
        .bind(organization_id)
        .bind(org_role)
        .execute(&mut *tx)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to revoke role rule: {e}")))?;

    tx.commit()
        .await
        .map_err(|e| ServerFnError::new(format!("Commit failed: {e}")))?;

    Ok(())
}

#[server]
pub async fn get_project_org_role_access(
    project_id: i64,
) -> Result<Vec<ProjectOrgRoleRule>, ServerFnError> {
    let pool = get_db().await;
    let rules = sqlx::query_as::<_, ProjectOrgRoleRule>(
        "SELECT project_id, organization_id, org_role, project_role
         FROM project_org_role_access
         WHERE project_id = $1
         ORDER BY organization_id ASC, org_role ASC",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to fetch role rules: {e}")))?;

    Ok(rules)
}

#[server]
pub async fn get_user_project_grants(
    project_id: i64,
) -> Result<Vec<ProjectMemberGrant>, ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;

    let grants = sqlx::query_as::<_, ProjectMemberGrant>(
        "SELECT pmg.project_id, pmg.user_id, pmg.grant_key, pmg.project_role,
                pmg.source_org_id, o.name AS source_org_name, pmg.source_org_role, pmg.granted_at
         FROM project_member_grants pmg
         LEFT JOIN organization o ON pmg.source_org_id = o.id
         WHERE pmg.project_id = $1 AND pmg.user_id = $2
         ORDER BY pmg.granted_at ASC",
    )
    .bind(project_id)
    .bind(user.id)
    .fetch_all(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to fetch user grants: {e}")))?;

    Ok(grants)
}

#[server]
pub async fn set_active_project_hat(
    project_id: i64,
    grant_key: Option<String>,
) -> Result<(), ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ServerFnError::new(format!("Transaction failed: {e}")))?;

    match grant_key {
        Some(key) => {
            let role_opt: Option<(String, i64)> = sqlx::query_as(
                "SELECT pmg.project_role, prd.permissions_mask
                 FROM project_member_grants pmg
                 JOIN project_role_definition prd ON prd.role = pmg.project_role
                 WHERE pmg.project_id = $1 AND pmg.user_id = $2 AND pmg.grant_key = $3",
            )
            .bind(project_id)
            .bind(user.id)
            .bind(&key)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?;

            let (role, mask) = match role_opt {
                Some(r) => r,
                None => return Err(ServerFnError::new("Selected hat grant not found.")),
            };

            sqlx::query(
                "UPDATE project_members
                 SET active_grant_key = $1, permissions_mask = $2, role = $3
                 WHERE project_id = $4 AND user_id = $5",
            )
            .bind(key)
            .bind(mask)
            .bind(role)
            .bind(project_id)
            .bind(user.id)
            .execute(&mut *tx)
            .await
            .map_err(|e| ServerFnError::new(format!("Failed to set active hat: {e}")))?;
        }
        None => {
            // Reset to union of all grants
            sqlx::query(
                "UPDATE project_members
                 SET active_grant_key = NULL,
                     permissions_mask = (
                         SELECT (MAX(prd.permissions_mask & 1))
                              | (MAX(prd.permissions_mask & 2))
                              | (MAX(prd.permissions_mask & 4))
                              | (MAX(prd.permissions_mask & 8))
                              | (MAX(prd.permissions_mask & 16))
                              | (MAX(prd.permissions_mask & 32))
                         FROM project_member_grants pmg
                         JOIN project_role_definition prd ON prd.role = pmg.project_role
                         WHERE pmg.project_id = $1 AND pmg.user_id = $2
                     )
                 WHERE project_id = $1 AND user_id = $2",
            )
            .bind(project_id)
            .bind(user.id)
            .execute(&mut *tx)
            .await
            .map_err(|e| ServerFnError::new(format!("Failed to reset hat: {e}")))?;

            sqlx::query(
                "UPDATE project_members
                 SET role = CASE
                     WHEN permissions_mask & 32 != 0 THEN 'admin'
                     WHEN permissions_mask & 4 != 0 THEN 'member'
                     ELSE 'viewer'
                 END
                 WHERE project_id = $1 AND user_id = $2",
            )
            .bind(project_id)
            .bind(user.id)
            .execute(&mut *tx)
            .await
            .map_err(|e| ServerFnError::new(format!("Failed to update role: {e}")))?;
        }
    }

    tx.commit()
        .await
        .map_err(|e| ServerFnError::new(format!("Commit failed: {e}")))?;

    Ok(())
}


