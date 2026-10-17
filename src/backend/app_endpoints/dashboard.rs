use dioxus::prelude::*;
#[allow(unused_imports)]
use crate::types::{
    DashboardData, GovernanceTransferOffer, Organization, OrganizationInvitation,
    Project, ProjectInvitation, ProjectLeadershipTransferOffer, UserInbox,
};

#[cfg(feature = "server")]
use crate::backend::app_endpoints::auth::get_auth_user;
#[cfg(feature = "server")]
use crate::backend::database::get_db;

#[server]
pub async fn get_user_dashboard_data() -> Result<DashboardData, ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;

    // 1. Projects
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

    // 2. Organizations
    let organizations = sqlx::query_as::<_, Organization>(
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

    // 3. Inbox - Project Invitations
    #[derive(sqlx::FromRow)]
    struct RawProjInv {
        project_id: i64,
        project_name: String,
        invited_user_id: i64,
        invited_by: Option<i64>,
        invited_by_name: Option<String>,
        role: String,
        message: String,
        created_at: i64,
    }

    let raw_proj_inv = sqlx::query_as::<_, RawProjInv>(
        "SELECT pi.project_id, p.name AS project_name, pi.invited_user_id,
                pi.invited_by, COALESCE(u.name, 'System') AS invited_by_name,
                pi.role, pi.message, pi.created_at
         FROM project_invitation pi
         INNER JOIN project p ON pi.project_id = p.id
         LEFT JOIN user u ON pi.invited_by = u.id
         WHERE pi.invited_user_id = $1
         ORDER BY pi.created_at DESC",
    )
    .bind(user.id)
    .fetch_all(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to fetch project invitations: {e}")))?;

    let project_invitations = raw_proj_inv
        .into_iter()
        .map(|r| ProjectInvitation {
            project_id: r.project_id,
            project_name: r.project_name,
            invited_user_id: r.invited_user_id,
            invited_by: r.invited_by,
            invited_by_name: r.invited_by_name.unwrap_or_else(|| "System".to_string()),
            role: r.role,
            message: r.message,
            created_at: r.created_at,
        })
        .collect();

    // 4. Inbox - Organization Invitations
    #[derive(sqlx::FromRow)]
    struct RawOrgInv {
        organization_id: i64,
        organization_name: String,
        invited_user_id: i64,
        invited_by: Option<i64>,
        invited_by_name: Option<String>,
        role: String,
        message: String,
        created_at: i64,
    }

    let raw_org_inv = sqlx::query_as::<_, RawOrgInv>(
        "SELECT oi.organization_id, o.name AS organization_name, oi.invited_user_id,
                oi.invited_by, COALESCE(u.name, 'System') AS invited_by_name,
                oi.role, oi.message, oi.created_at
         FROM organization_invitation oi
         INNER JOIN organization o ON oi.organization_id = o.id
         LEFT JOIN user u ON oi.invited_by = u.id
         WHERE oi.invited_user_id = $1
         ORDER BY oi.created_at DESC",
    )
    .bind(user.id)
    .fetch_all(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to fetch org invitations: {e}")))?;

    let org_invitations = raw_org_inv
        .into_iter()
        .map(|r| OrganizationInvitation {
            organization_id: r.organization_id,
            organization_name: r.organization_name,
            invited_user_id: r.invited_user_id,
            invited_by: r.invited_by,
            invited_by_name: r.invited_by_name.unwrap_or_else(|| "System".to_string()),
            role: r.role,
            message: r.message,
            created_at: r.created_at,
        })
        .collect();

    // 5. Inbox - Governance Transfers
    #[derive(sqlx::FromRow)]
    struct RawGovTrans {
        organization_id: i64,
        organization_name: String,
        requested_new_governor: i64,
        current_governor_name: String,
        created_at: i64,
    }

    let raw_gov_trans = sqlx::query_as::<_, RawGovTrans>(
        "SELECT ogt.organization_id, o.name AS organization_name,
                ogt.requested_new_governor, u.name AS current_governor_name,
                ogt.created_at
         FROM organization_governance_transfer ogt
         INNER JOIN organization o ON ogt.organization_id = o.id
         INNER JOIN user u ON o.governing_user_id = u.id
         WHERE ogt.requested_new_governor = $1
         ORDER BY ogt.created_at DESC",
    )
    .bind(user.id)
    .fetch_all(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to fetch governance transfers: {e}")))?;

    let governance_transfers = raw_gov_trans
        .into_iter()
        .map(|r| GovernanceTransferOffer {
            organization_id: r.organization_id,
            organization_name: r.organization_name,
            requested_new_governor: r.requested_new_governor,
            current_governor_name: r.current_governor_name,
            created_at: r.created_at,
        })
        .collect();

    // 6. Inbox - Leadership Transfers
    #[derive(sqlx::FromRow)]
    struct RawLeadTrans {
        project_id: i64,
        project_name: String,
        requested_new_leader: i64,
        current_leader_name: String,
        created_at: i64,
    }

    let raw_lead_trans = sqlx::query_as::<_, RawLeadTrans>(
        "SELECT plt.project_id, p.name AS project_name,
                plt.requested_new_leader, u.name AS current_leader_name,
                plt.created_at
         FROM project_leadership_transfer plt
         INNER JOIN project p ON plt.project_id = p.id
         INNER JOIN user u ON p.leader_id = u.id
         WHERE plt.requested_new_leader = $1
         ORDER BY plt.created_at DESC",
    )
    .bind(user.id)
    .fetch_all(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to fetch leadership transfers: {e}")))?;

    let leadership_transfers = raw_lead_trans
        .into_iter()
        .map(|r| ProjectLeadershipTransferOffer {
            project_id: r.project_id,
            project_name: r.project_name,
            requested_new_leader: r.requested_new_leader,
            current_leader_name: r.current_leader_name,
            created_at: r.created_at,
        })
        .collect();

    Ok(DashboardData {
        inbox: UserInbox {
            project_invitations,
            org_invitations,
            governance_transfers,
            leadership_transfers,
        },
        projects,
        organizations,
    })
}
