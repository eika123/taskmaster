use crate::types::Permission;
use dioxus::prelude::*;
use sqlx::PgPool;

pub async fn check_project_perm(
    pool: &PgPool,
    project_id: i64,
    user_id: i64,
    required_mask: i64,
) -> Result<bool, ServerFnError> {
    sqlx::query_scalar(
        "SELECT (p.leader_id = $1 OR EXISTS (
            SELECT 1 FROM project_members pm
            WHERE pm.project_id = p.id AND pm.user_id = $1 AND has_perm(pm.permissions_mask, $2)
         ))
         FROM project p WHERE p.id = $3",
    )
    .bind(user_id)
    .bind(required_mask)
    .bind(project_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Database error: {e}")))
    .map(|opt| opt.unwrap_or(false))
}

pub async fn ensure_project_perm(
    pool: &PgPool,
    project_id: i64,
    user_id: i64,
    required_mask: i64,
    err_msg: &str,
) -> Result<(), ServerFnError> {
    if !check_project_perm(pool, project_id, user_id, required_mask).await? {
        return Err(ServerFnError::new(err_msg));
    }
    Ok(())
}

pub async fn can_view_project(
    pool: &PgPool,
    project_id: i64,
    user_id: i64,
) -> Result<bool, ServerFnError> {
    sqlx::query_scalar(
        "SELECT (
            p.leader_id = $1
            OR EXISTS (
                SELECT 1 FROM project_members pm
                WHERE pm.project_id = p.id AND pm.user_id = $1 AND has_perm(pm.permissions_mask, $2)
            )
            OR (p.owner_org_id IS NOT NULL AND EXISTS (
                SELECT 1 FROM organization o
                WHERE o.id = p.owner_org_id AND (
                    o.governing_user_id = $1
                    OR EXISTS (
                        SELECT 1 FROM organization_member_roles omr
                        WHERE omr.organization_id = o.id AND omr.user_id = $1 AND omr.role = 'admin'
                    )
                )
            ))
         )
         FROM project p WHERE p.id = $3",
    )
    .bind(user_id)
    .bind(Permission::READ.as_i64())
    .bind(project_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Database error: {e}")))
    .map(|opt| opt.unwrap_or(false))
}

pub async fn is_org_admin(pool: &PgPool, org_id: i64, user_id: i64) -> Result<bool, ServerFnError> {
    sqlx::query_scalar(
        "SELECT (o.governing_user_id = $1 OR EXISTS (
            SELECT 1 FROM organization_member_roles omr
            WHERE omr.organization_id = o.id AND omr.user_id = $1 AND omr.role = 'admin'
         ))
         FROM organization o WHERE o.id = $2",
    )
    .bind(user_id)
    .bind(org_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Database error: {e}")))
    .map(|opt| opt.unwrap_or(false))
}

pub async fn ensure_org_admin(
    pool: &PgPool,
    org_id: i64,
    user_id: i64,
    err_msg: &str,
) -> Result<(), ServerFnError> {
    if !is_org_admin(pool, org_id, user_id).await? {
        return Err(ServerFnError::new(err_msg));
    }
    Ok(())
}

pub fn verify_confirmation(
    confirmation: &str,
    entity_name: &str,
    entity_type: &str,
) -> Result<(), ServerFnError> {
    let expected = format!("DELETE {}", entity_name.trim());
    if confirmation.trim() != expected {
        return Err(ServerFnError::new(format!(
            "Confirmation mismatch. Please type exactly '{expected}' to delete this {entity_type}."
        )));
    }
    Ok(())
}
