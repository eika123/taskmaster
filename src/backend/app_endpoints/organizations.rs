use dioxus::prelude::*;
use crate::types::{Organization, OrganizationMember};

#[cfg(feature = "server")]
use crate::backend::app_endpoints::auth::get_auth_user;
#[cfg(feature = "server")]
use crate::backend::database::get_db;

#[server]
pub async fn get_user_organizations() -> Result<Vec<Organization>, ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;

    let orgs = sqlx::query_as::<_, Organization>(
        "SELECT o.id, o.name, o.slug, o.governing_user_id, u.name AS governing_user_name, o.created_at
         FROM organization o
         INNER JOIN user u ON o.governing_user_id = u.id
         WHERE o.governing_user_id = $1
            OR EXISTS (SELECT 1 FROM organization_members om WHERE om.organization_id = o.id AND om.user_id = $1)
         ORDER BY o.id ASC",
    )
    .bind(user.id)
    .fetch_all(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to fetch organizations: {e}")))?;

    Ok(orgs)
}

#[server]
pub async fn create_organization(
    name: String,
    slug: String,
) -> Result<Organization, ServerFnError> {
    let user = get_auth_user().await?;
    let name_clean = name.trim();
    let slug_clean = slug.trim().to_lowercase();

    if name_clean.is_empty() {
        return Err(ServerFnError::new("Organization name is required."));
    }
    if slug_clean.is_empty() {
        return Err(ServerFnError::new("Organization slug is required."));
    }

    let pool = get_db().await;
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ServerFnError::new(format!("Transaction failed: {e}")))?;

    let res = sqlx::query(
        "INSERT INTO organization (name, slug, governing_user_id) VALUES ($1, $2, $3)",
    )
    .bind(name_clean)
    .bind(&slug_clean)
    .bind(user.id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to create organization: {e}")))?;

    let org_id = res.last_insert_rowid();

    sqlx::query(
        "INSERT INTO organization_members (organization_id, user_id, role) VALUES ($1, $2, 'admin')",
    )
    .bind(org_id)
    .bind(user.id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to register governor: {e}")))?;

    tx.commit()
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to commit organization: {e}")))?;

    let org = sqlx::query_as::<_, Organization>(
        "SELECT o.id, o.name, o.slug, o.governing_user_id, u.name AS governing_user_name, o.created_at
         FROM organization o
         INNER JOIN user u ON o.governing_user_id = u.id
         WHERE o.id = $1",
    )
    .bind(org_id)
    .fetch_one(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to fetch new organization: {e}")))?;

    Ok(org)
}

#[server]
pub async fn delete_organization(organization_id: i64) -> Result<(), ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;

    let governing_id: i64 =
        sqlx::query_scalar("SELECT governing_user_id FROM organization WHERE id = $1")
            .bind(organization_id)
            .fetch_one(pool)
            .await
            .map_err(|e| ServerFnError::new(format!("Organization not found: {e}")))?;

    if governing_id != user.id {
        return Err(ServerFnError::new(
            "Only the governing user can delete the organization.",
        ));
    }

    sqlx::query("DELETE FROM organization WHERE id = $1")
        .bind(organization_id)
        .execute(pool)
        .await
        .map_err(|e| {
            ServerFnError::new(format!(
                "Cannot delete organization: {e}. Remove all other members first."
            ))
        })?;

    Ok(())
}

#[server]
pub async fn get_organization_members(
    organization_id: i64,
) -> Result<Vec<OrganizationMember>, ServerFnError> {
    let pool = get_db().await;
    let members = sqlx::query_as::<_, OrganizationMember>(
        "SELECT om.organization_id, om.user_id, u.name AS user_name, u.email AS user_email,
                om.role, om.joined_at
         FROM organization_members om
         INNER JOIN user u ON om.user_id = u.id
         WHERE om.organization_id = $1
         ORDER BY u.name ASC",
    )
    .bind(organization_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to fetch organization members: {e}")))?;

    Ok(members)
}

#[server]
pub async fn remove_organization_member(
    organization_id: i64,
    member_user_id: i64,
) -> Result<(), ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;

    let governing_id: i64 =
        sqlx::query_scalar("SELECT governing_user_id FROM organization WHERE id = $1")
            .bind(organization_id)
            .fetch_one(pool)
            .await
            .map_err(|e| ServerFnError::new(format!("Organization not found: {e}")))?;

    if user.id != member_user_id && user.id != governing_id {
        return Err(ServerFnError::new(
            "Only the organization governor can remove other members.",
        ));
    }

    // The database trigger `prevent_remove_governing_user` will automatically block
    // the governing user from removing themselves unless governance is transferred first!
    sqlx::query("DELETE FROM organization_members WHERE organization_id = $1 AND user_id = $2")
        .bind(organization_id)
        .bind(member_user_id)
        .execute(pool)
        .await
        .map_err(|e| ServerFnError::new(format!("Cannot remove member: {e}")))?;

    Ok(())
}

#[server]
pub async fn invite_user_to_organization(
    organization_id: i64,
    invited_user_id: i64,
    role: String,
    message: String,
) -> Result<(), ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;

    sqlx::query(
        "INSERT INTO organization_invitation (organization_id, invited_user_id, invited_by, role, message)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT(organization_id, invited_user_id) DO UPDATE SET
            role = excluded.role,
            message = excluded.message,
            created_at = unixepoch()",
    )
    .bind(organization_id)
    .bind(invited_user_id)
    .bind(user.id)
    .bind(role)
    .bind(message.trim())
    .execute(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to send organization invitation: {e}")))?;

    Ok(())
}

#[server]
pub async fn respond_to_organization_invitation(
    organization_id: i64,
    accept: bool,
) -> Result<(), ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;

    let invite: Option<(String,)> = sqlx::query_as(
        "SELECT role FROM organization_invitation WHERE organization_id = $1 AND invited_user_id = $2",
    )
    .bind(organization_id)
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

    sqlx::query(
        "DELETE FROM organization_invitation WHERE organization_id = $1 AND invited_user_id = $2",
    )
    .bind(organization_id)
    .bind(user.id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to clear invite: {e}")))?;

    if accept {
        sqlx::query(
            "INSERT INTO organization_members (organization_id, user_id, role)
             VALUES ($1, $2, $3)
             ON CONFLICT(organization_id, user_id) DO UPDATE SET role = excluded.role",
        )
        .bind(organization_id)
        .bind(user.id)
        .bind(role)
        .execute(&mut *tx)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to join organization: {e}")))?;
    }

    tx.commit()
        .await
        .map_err(|e| ServerFnError::new(format!("Commit failed: {e}")))?;

    Ok(())
}

/// Initiates governance transfer / stepping down as governor.
///
/// When the current governing user wishes to step down (seek relief from governing duties),
/// they cannot simply leave or remove themselves due to the database trigger `prevent_remove_governing_user`.
/// Instead, they nominate a successor member via this endpoint. The designated member receives a transfer
/// offer in their inbox, and only upon acceptance does the governance role atomically transfer to the successor.
#[server]
pub async fn request_organization_governance_transfer(
    organization_id: i64,
    requested_new_governor: i64,
) -> Result<(), ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;

    let gov_id: i64 = sqlx::query_scalar("SELECT governing_user_id FROM organization WHERE id = $1")
        .bind(organization_id)
        .fetch_one(pool)
        .await
        .map_err(|e| ServerFnError::new(format!("Organization not found: {e}")))?;

    if gov_id != user.id {
        return Err(ServerFnError::new("Only current governor can initiate transfer."));
    }

    sqlx::query(
        "INSERT INTO organization_governance_transfer (organization_id, requested_new_governor, created_at)
         VALUES ($1, $2, unixepoch())
         ON CONFLICT(organization_id) DO UPDATE SET
            requested_new_governor = excluded.requested_new_governor,
            created_at = unixepoch()",
    )
    .bind(organization_id)
    .bind(requested_new_governor)
    .execute(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to request governance transfer: {e}")))?;

    Ok(())
}


#[server]
pub async fn respond_to_organization_governance_transfer_request(
    organization_id: i64,
    accept: bool,
) -> Result<(), ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;

    let target: Option<(i64,)> = sqlx::query_as(
        "SELECT requested_new_governor FROM organization_governance_transfer WHERE organization_id = $1",
    )
    .bind(organization_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Database error: {e}")))?;

    if target.map(|(t,)| t) != Some(user.id) {
        return Err(ServerFnError::new("No pending governance transfer for you."));
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ServerFnError::new(format!("Transaction error: {e}")))?;

    sqlx::query("DELETE FROM organization_governance_transfer WHERE organization_id = $1")
        .bind(organization_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to clear transfer request: {e}")))?;

    if accept {
        sqlx::query("UPDATE organization SET governing_user_id = $1 WHERE id = $2")
            .bind(user.id)
            .bind(organization_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| ServerFnError::new(format!("Failed to transfer governance: {e}")))?;
    }

    tx.commit()
        .await
        .map_err(|e| ServerFnError::new(format!("Commit failed: {e}")))?;

    Ok(())
}

