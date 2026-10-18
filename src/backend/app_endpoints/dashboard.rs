use dioxus::prelude::*;
use crate::types::DashboardData;

#[cfg(feature = "server")]
use crate::types::{
    GovernanceTransferOffer, OrganizationInvitation,
    ProjectInvitation, ProjectLeadershipTransferOffer, UserInbox,
};

#[cfg(feature = "server")]
use crate::backend::app_endpoints::auth::get_auth_user;
#[cfg(feature = "server")]
use crate::backend::app_endpoints::organizations::get_user_organizations;
#[cfg(feature = "server")]
use crate::backend::app_endpoints::projects::get_user_projects;
#[cfg(feature = "server")]
use crate::backend::database::get_db;

#[server]
pub async fn get_user_dashboard_data() -> Result<DashboardData, ServerFnError> {
    let user = get_auth_user().await?;
    let pool = get_db().await;

    let projects = get_user_projects().await?;
    let organizations = get_user_organizations().await?;

    let project_invitations = sqlx::query_as::<_, ProjectInvitation>(
        "SELECT pi.project_id, p.name AS project_name, pi.invited_user_id,
                pi.invited_by, COALESCE(u.name, 'System') AS invited_by_name,
                pi.role, pi.message, pi.created_at
         FROM project_invitation pi
         INNER JOIN project p ON pi.project_id = p.id
         LEFT JOIN \"user\" u ON pi.invited_by = u.id
         WHERE pi.invited_user_id = $1
         ORDER BY pi.created_at DESC",
    )
    .bind(user.id)
    .fetch_all(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to fetch project invitations: {e}")))?;

    let org_invitations = sqlx::query_as::<_, OrganizationInvitation>(
        "SELECT oi.organization_id, o.name AS organization_name, oi.invited_user_id,
                oi.invited_by, COALESCE(u.name, 'System') AS invited_by_name,
                oi.role, oi.message, oi.created_at
         FROM organization_invitation oi
         INNER JOIN organization o ON oi.organization_id = o.id
         LEFT JOIN \"user\" u ON oi.invited_by = u.id
         WHERE oi.invited_user_id = $1
         ORDER BY oi.created_at DESC",
    )
    .bind(user.id)
    .fetch_all(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to fetch org invitations: {e}")))?;

    let governance_transfers = sqlx::query_as::<_, GovernanceTransferOffer>(
        "SELECT ogt.organization_id, o.name AS organization_name,
                ogt.requested_new_governor, u.name AS current_governor_name,
                ogt.created_at
         FROM organization_governance_transfer ogt
         INNER JOIN organization o ON ogt.organization_id = o.id
         INNER JOIN \"user\" u ON o.governing_user_id = u.id
         WHERE ogt.requested_new_governor = $1
         ORDER BY ogt.created_at DESC",
    )
    .bind(user.id)
    .fetch_all(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to fetch governance transfers: {e}")))?;

    let leadership_transfers = sqlx::query_as::<_, ProjectLeadershipTransferOffer>(
        "SELECT plt.project_id, p.name AS project_name,
                plt.requested_new_leader, u.name AS current_leader_name,
                plt.created_at
         FROM project_leadership_transfer plt
         INNER JOIN project p ON plt.project_id = p.id
         INNER JOIN \"user\" u ON p.leader_id = u.id
         WHERE plt.requested_new_leader = $1
         ORDER BY plt.created_at DESC",
    )
    .bind(user.id)
    .fetch_all(pool)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to fetch leadership transfers: {e}")))?;

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