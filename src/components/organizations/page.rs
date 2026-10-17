use dioxus::prelude::*;
use crate::Route;
use crate::backend::app_endpoints::{
    get_organization, get_organization_affiliated_projects, get_organization_members,
    get_organization_pending_invitations, get_organization_roles, get_users,
};
use super::governor_view::OrganizationGovernorView;
use super::member_view::OrganizationMemberView;

#[component]
pub fn OrganizationDashboardPage(organization_id: i64) -> Element {
    let mut org_resource = use_server_future(move || async move {
        get_organization(organization_id).await
    })?;

    let mut members_resource = use_server_future(move || async move {
        get_organization_members(organization_id).await
    })?;

    let mut roles_resource = use_server_future(move || async move {
        get_organization_roles(organization_id).await
    })?;

    let mut projects_resource = use_server_future(move || async move {
        get_organization_affiliated_projects(organization_id).await
    })?;

    let mut invites_resource = use_server_future(move || async move {
        get_organization_pending_invitations(organization_id).await
    })?;

    let users_resource = use_server_future(|| async move {
        get_users().await
    })?;

    let auth = use_context::<crate::components::AuthState>();
    let current_user_id = auth.user.read().as_ref().map(|u| u.id).unwrap_or(0);

    let organization = match &*org_resource.read() {
        Some(Ok(org)) => org.clone(),
        Some(Err(err)) => {
            return rsx! {
                div { class: "dashboard-container",
                    div { class: "task-detail-error", "Failed to load organization: {err}" }
                    div { style: "margin-top: 16px;",
                        Link { to: Route::DashboardPage {}, class: "btn-task-cancel", "← Back to Dashboard" }
                    }
                }
            };
        }
        None => {
            return rsx! {
                div { class: "dashboard-container",
                    p { "Loading organization..." }
                }
            };
        }
    };

    let members = match &*members_resource.read() {
        Some(Ok(m)) => m.clone(),
        _ => Vec::new(),
    };

    let roles = match &*roles_resource.read() {
        Some(Ok(r)) => r.clone(),
        _ => Vec::new(),
    };

    let affiliated_projects = match &*projects_resource.read() {
        Some(Ok(p)) => p.clone(),
        _ => Vec::new(),
    };

    let pending_invitations = match &*invites_resource.read() {
        Some(Ok(inv)) => inv.clone(),
        _ => Vec::new(),
    };

    let all_users = match &*users_resource.read() {
        Some(Ok(u)) => u.clone(),
        _ => Vec::new(),
    };

    let is_governor = organization.governing_user_id == current_user_id;

    let handle_refresh = move |_| {
        org_resource.restart();
        members_resource.restart();
        roles_resource.restart();
        projects_resource.restart();
        invites_resource.restart();
    };

    rsx! {
        div { class: "dashboard-container",
            div { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 20px;",
                Link {
                    to: Route::DashboardPage {},
                    style: "color: var(--primary); text-decoration: none; font-size: var(--font-size-body-sm); font-weight: var(--font-weight-bold); display: inline-flex; align-items: center; gap: 6px;",
                    "← Back to Dashboard"
                }
                span { style: "color: var(--text-muted); font-size: var(--font-size-meta);",
                    if is_governor { "Viewing as: Governor" } else { "Viewing as: Organization Member" }
                }
            }

            if is_governor {
                OrganizationGovernorView {
                    organization,
                    members,
                    roles,
                    affiliated_projects,
                    pending_invitations,
                    all_users,
                    on_refresh: handle_refresh,
                }
            } else {
                OrganizationMemberView {
                    organization,
                    members,
                    affiliated_projects,
                    current_user_id,
                }
            }
        }
    }
}

