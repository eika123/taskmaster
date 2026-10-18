use super::modals::{
    AssignMemberRoleModal, ConfirmActiveTasksWarningModal, ConfirmRemoveMemberModal,
    CreateOrgProjectModal, CreateOrgRoleModal, InviteOrgMemberModal, TransferOrgGovernanceModal,
};
use crate::backend::app_endpoints::{
    cancel_organization_invitation, delete_organization, remove_organization_member,
    revoke_organization_member_role,
};
use crate::components::dashboard::ConfirmDeleteModal;
use crate::types::{
    OrgProjectAffiliation, Organization, OrganizationMember, OrganizationPendingInvite, User,
};
use crate::Route;
use dioxus::prelude::*;

#[derive(Clone, PartialEq, Eq)]
enum PendingWarningAction {
    RevokeRole { user_id: i64, role: String },
    RevokeMember { user_id: i64 },
}

#[component]
pub fn OrganizationGovernorView(
    organization: Organization,
    members: Vec<OrganizationMember>,
    roles: Vec<String>,
    affiliated_projects: Vec<OrgProjectAffiliation>,
    pending_invitations: Vec<OrganizationPendingInvite>,
    all_users: Vec<User>,
    on_refresh: EventHandler<()>,
) -> Element {
    let org_id = organization.id;
    let org_name = organization.name.clone();
    let current_gov_id = organization.governing_user_id;

    // Modals state
    let mut show_invite_modal = use_signal(|| false);
    let mut show_create_project_modal = use_signal(|| false);
    let mut show_create_role_modal = use_signal(|| false);
    let mut show_transfer_modal = use_signal(|| false);
    let mut show_delete_modal = use_signal(|| false);
    let mut active_assign_member = use_signal(|| Option::<OrganizationMember>::None);
    let mut member_to_remove = use_signal(|| Option::<OrganizationMember>::None);
    let mut is_removing_member = use_signal(|| false);

    // Warning confirmation modal state for active tasks
    let mut pending_warning = use_signal(|| Option::<(String, PendingWarningAction)>::None);
    let mut is_warning_loading = use_signal(|| false);

    // Delete organization state
    let mut is_deleting = use_signal(|| false);
    let mut delete_error = use_signal(|| Option::<String>::None);
    let nav = use_navigator();

    // Handler to revoke member role
    let handle_revoke_member_role = move |(uid, role_name): (i64, String)| {
        let r_name_clone = role_name.clone();
        spawn(async move {
            match revoke_organization_member_role(org_id, uid, role_name.clone(), false).await {
                Ok(_) => on_refresh.call(()),
                Err(e) => {
                    let err_str = e.to_string();
                    if err_str.contains("Active task impact warning") {
                        pending_warning.set(Some((
                            err_str,
                            PendingWarningAction::RevokeRole {
                                user_id: uid,
                                role: r_name_clone,
                            },
                        )));
                    }
                }
            }
        });
    };

    // Handler to remove member
    let handle_confirm_remove_member = move || {
        if let Some(m) = member_to_remove.read().clone() {
            let uid = m.user_id;
            is_removing_member.set(true);
            spawn(async move {
                match remove_organization_member(org_id, uid, false).await {
                    Ok(_) => {
                        is_removing_member.set(false);
                        member_to_remove.set(None);
                        on_refresh.call(());
                    }
                    Err(e) => {
                        is_removing_member.set(false);
                        member_to_remove.set(None);
                        let err_str = e.to_string();
                        if err_str.contains("Active task impact warning") {
                            pending_warning.set(Some((
                                err_str,
                                PendingWarningAction::RevokeMember { user_id: uid },
                            )));
                        }
                    }
                }
            });
        }
    };

    // Handler to cancel pending invitation
    let handle_cancel_invite = move |invited_uid: i64| {
        spawn(async move {
            if cancel_organization_invitation(org_id, invited_uid)
                .await
                .is_ok()
            {
                on_refresh.call(());
            }
        });
    };

    let handle_confirm_warning = move |_| {
        if let Some((_, action)) = (*pending_warning.read()).clone() {
            is_warning_loading.set(true);
            spawn(async move {
                match action {
                    PendingWarningAction::RevokeRole { user_id, role } => {
                        let _ = revoke_organization_member_role(org_id, user_id, role, true).await;
                    }
                    PendingWarningAction::RevokeMember { user_id } => {
                        let _ = remove_organization_member(org_id, user_id, true).await;
                    }
                }
                is_warning_loading.set(false);
                pending_warning.set(None);
                on_refresh.call(());
            });
        }
    };

    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 24px;",
            GovernorHeaderCard {
                organization: organization.clone(),
                member_count: members.len(),
                role_count: roles.len(),
                project_count: affiliated_projects.len(),
                invite_count: pending_invitations.len(),
                on_new_project: move |_| show_create_project_modal.set(true),
                on_invite_member: move |_| show_invite_modal.set(true),
            }

            GovernorProjectsSection {
                organization_name: org_name.clone(),
                affiliated_projects: affiliated_projects.clone(),
                on_create_project: move |_| show_create_project_modal.set(true),
            }

            if !pending_invitations.is_empty() {
                GovernorPendingInvitesSection {
                    pending_invitations: pending_invitations.clone(),
                    on_cancel_invite: handle_cancel_invite,
                }
            }

            GovernorMembersSection {
                organization_name: org_name.clone(),
                org_id,
                members: members.clone(),
                current_gov_id,
                on_assign_role: move |m| active_assign_member.set(Some(m)),
                on_remove_member: move |m| member_to_remove.set(Some(m)),
                on_revoke_role: handle_revoke_member_role,
            }

            GovernorDangerZoneSection {
                on_transfer: move |_| show_transfer_modal.set(true),
                on_delete: move |_| show_delete_modal.set(true),
            }

            GovernorModalsHost {
                org_id,
                org_name: org_name.clone(),
                current_gov_id,
                roles: roles.clone(),
                all_users: all_users.clone(),
                members: members.clone(),
                member_to_remove: member_to_remove.read().clone(),
                is_removing_member: *is_removing_member.read(),
                pending_warning: (*pending_warning.read()).clone(),
                is_warning_loading: *is_warning_loading.read(),
                show_invite_modal: *show_invite_modal.read(),
                show_create_project_modal: *show_create_project_modal.read(),
                show_create_role_modal: *show_create_role_modal.read(),
                active_assign_member: active_assign_member.read().clone(),
                show_transfer_modal: *show_transfer_modal.read(),
                show_delete_modal: *show_delete_modal.read(),
                is_deleting: *is_deleting.read(),
                delete_error: (*delete_error.read()).clone(),
                on_close_remove_member: move |_| member_to_remove.set(None),
                on_confirm_remove_member: handle_confirm_remove_member,
                on_close_warning: move |_| pending_warning.set(None),
                on_confirm_warning: handle_confirm_warning,
                on_close_invite: move |_| show_invite_modal.set(false),
                on_close_create_project: move |_| show_create_project_modal.set(false),
                on_close_create_role: move |_| show_create_role_modal.set(false),
                on_close_assign: move |_| active_assign_member.set(None),
                on_close_transfer: move |_| show_transfer_modal.set(false),
                on_close_delete: move |_| {
                    show_delete_modal.set(false);
                    delete_error.set(None);
                },
                on_confirm_delete: move |confirmation: String| {
                    is_deleting.set(true);
                    delete_error.set(None);
                    let nav_clone = nav.clone();
                    spawn(async move {
                        match delete_organization(org_id, confirmation).await {
                            Ok(_) => {
                                is_deleting.set(false);
                                show_delete_modal.set(false);
                                nav_clone.push(Route::DashboardPage {});
                            }
                            Err(err) => {
                                is_deleting.set(false);
                                delete_error.set(Some(err.to_string()));
                            }
                        }
                    });
                },
                on_refresh: move |_| on_refresh.call(()),
            }
        }
    }
}

/// Governor header card with organization metadata, actions, and key statistics tiles.
#[component]
fn GovernorHeaderCard(
    organization: Organization,
    member_count: usize,
    role_count: usize,
    project_count: usize,
    invite_count: usize,
    on_new_project: EventHandler<()>,
    on_invite_member: EventHandler<()>,
) -> Element {
    let org_id = organization.id;
    rsx! {
        div { class: "dashboard-panel",
            div { style: "display: flex; justify-content: space-between; align-items: flex-start; flex-wrap: wrap; gap: 16px;",
                div {
                    div { style: "display: flex; gap: 10px; align-items: center; margin-bottom: 4px;",
                        h1 { class: "dashboard-title", style: "margin: 0;", "{organization.name}" }
                        span { class: "task-detail-status status-accepted badge-sm", "Governor" }
                    }
                    p { style: "margin: 0; color: var(--text-secondary); font-size: var(--font-size-body-sm);",
                        "Slug: " strong { "@{organization.slug}" }
                        " • Governed by " strong { "{organization.governing_user_name}" }
                    }
                }
                div { style: "display: flex; gap: 10px; flex-wrap: wrap;",
                    Link {
                        to: Route::OrganizationRolesPage { organization_id: org_id },
                        class: "btn-task-edit",
                        style: "text-decoration: none;",
                        "🏷 Manage Roles & Rules →"
                    }
                    button {
                        r#type: "button",
                        class: "btn-task-save",
                        onclick: move |_| on_new_project.call(()),
                        "+ New Project"
                    }
                    button {
                        r#type: "button",
                        class: "btn-task-cancel",
                        onclick: move |_| on_invite_member.call(()),
                        "👥 + Invite Member"
                    }
                }
            }

            div { style: "display: grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); gap: 12px; margin-top: 8px;",
                div { style: "background-color: var(--bg-surface-raised); border: 1px solid var(--border-subtle); border-radius: var(--radius-sm); padding: 12px;",
                    div { style: "font-size: var(--font-size-meta); color: var(--text-muted); text-transform: uppercase; font-weight: var(--font-weight-bold);", "Members" }
                    div { style: "font-size: var(--font-size-containerheader-medium); font-weight: var(--font-weight-bold); color: var(--text-primary); margin-top: 4px;", "{member_count}" }
                }
                div { style: "background-color: var(--bg-surface-raised); border: 1px solid var(--border-subtle); border-radius: var(--radius-sm); padding: 12px;",
                    div { style: "font-size: var(--font-size-meta); color: var(--text-muted); text-transform: uppercase; font-weight: var(--font-weight-bold);", "Custom Roles" }
                    div { style: "font-size: var(--font-size-containerheader-medium); font-weight: var(--font-weight-bold); color: var(--text-primary); margin-top: 4px;", "{role_count}" }
                }
                div { style: "background-color: var(--bg-surface-raised); border: 1px solid var(--border-subtle); border-radius: var(--radius-sm); padding: 12px;",
                    div { style: "font-size: var(--font-size-meta); color: var(--text-muted); text-transform: uppercase; font-weight: var(--font-weight-bold);", "Active Projects" }
                    div { style: "font-size: var(--font-size-containerheader-medium); font-weight: var(--font-weight-bold); color: var(--text-primary); margin-top: 4px;", "{project_count}" }
                }
                div { style: "background-color: var(--bg-surface-raised); border: 1px solid var(--border-subtle); border-radius: var(--radius-sm); padding: 12px;",
                    div { style: "font-size: var(--font-size-meta); color: var(--text-muted); text-transform: uppercase; font-weight: var(--font-weight-bold);", "Pending Invites" }
                    div { style: "font-size: var(--font-size-containerheader-medium); font-weight: var(--font-weight-bold); color: var(--text-primary); margin-top: 4px;", "{invite_count}" }
                }
            }
        }
    }
}

/// Section displaying active projects affiliated with this organization.
#[component]
fn GovernorProjectsSection(
    organization_name: String,
    affiliated_projects: Vec<OrgProjectAffiliation>,
    on_create_project: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "dashboard-panel",
            div { style: "display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 10px;",
                div {
                    h2 { class: "dashboard-panel-title", style: "margin: 0;", "Active Organization Projects ({affiliated_projects.len()})" }
                    p { style: "margin: 4px 0 0; color: var(--text-muted); font-size: var(--font-size-body-sm);",
                        "Projects affiliated with or owned by this organization. Manage tasks or configure organization participation."
                    }
                }
                button {
                    r#type: "button",
                    class: "btn-action-sm",
                    onclick: move |_| on_create_project.call(()),
                    "+ Create Project for {organization_name}"
                }
            }

            if affiliated_projects.is_empty() {
                p { class: "empty-description", "No projects are affiliated with this organization yet. Create one above to get started!" }
            } else {
                div { class: "project-card-list",
                    for p in affiliated_projects {
                        div { key: "{p.project_id}", class: "project-card",
                            div { style: "display: flex; justify-content: space-between; align-items: flex-start;",
                                Link {
                                    to: Route::ProjectTasksPage { project_id: p.project_id },
                                    class: "project-card-title",
                                    "{p.project_name}"
                                }
                                span { class: "task-detail-status status-accepted badge-sm",
                                    "Rel: {p.relationship_type}"
                                }
                            }
                            div { class: "project-card-meta",
                                span { "Max Role Ceiling: " strong { "{p.max_granted_role}" } }
                                div { style: "display: flex; gap: 8px; align-items: center;",
                                    Link {
                                        to: Route::ProjectManagePage { project_id: p.project_id },
                                        class: "btn-action-sm",
                                        style: "text-decoration: none;",
                                        "⚙ Manage Access"
                                    }
                                    Link {
                                        to: Route::ProjectTasksPage { project_id: p.project_id },
                                        style: "color: var(--primary); text-decoration: none; font-weight: var(--font-weight-bold);",
                                        "Open Board →"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Section listing outgoing invitations that are still pending.
#[component]
fn GovernorPendingInvitesSection(
    pending_invitations: Vec<OrganizationPendingInvite>,
    on_cancel_invite: EventHandler<i64>,
) -> Element {
    rsx! {
        div { class: "dashboard-panel",
            h2 { class: "dashboard-panel-title", "Outgoing Pending Invitations ({pending_invitations.len()})" }
            p { style: "margin: 4px 0 0; color: var(--text-muted); font-size: var(--font-size-body-sm);",
                "Users who have been invited to join this organization but have not yet accepted."
            }
            div { style: "display: flex; flex-direction: column; gap: 10px; margin-top: 6px;",
                for inv in pending_invitations {
                    {
                        let target_uid = inv.invited_user_id;
                        rsx! {
                            div { key: "{target_uid}", class: "inbox-item",
                                div { style: "display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 10px;",
                                    div { style: "display: flex; gap: 10px; align-items: center;",
                                        span { class: "inbox-item-title", "{inv.invited_user_name}" }
                                        span { class: "inbox-item-meta", "{inv.invited_user_email}" }
                                        span { class: "task-detail-status status-suggested badge-sm", "Role: {inv.role}" }
                                    }
                                    button {
                                        r#type: "button",
                                        class: "btn-action-sm",
                                        style: "color: var(--danger); border-color: var(--danger);",
                                        onclick: move |_| on_cancel_invite.call(target_uid),
                                        "Cancel Invitation"
                                    }
                                }
                                if !inv.message.is_empty() {
                                    p { style: "margin: 4px 0 0; font-size: var(--font-size-body-sm); color: var(--text-secondary);",
                                        "Note: \"{inv.message}\""
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Section listing organization members with role assignment and removal actions.
#[component]
fn GovernorMembersSection(
    organization_name: String,
    org_id: i64,
    members: Vec<OrganizationMember>,
    current_gov_id: i64,
    on_assign_role: EventHandler<OrganizationMember>,
    on_remove_member: EventHandler<OrganizationMember>,
    on_revoke_role: EventHandler<(i64, String)>,
) -> Element {
    rsx! {
        div { class: "dashboard-panel",
            div { style: "display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 10px;",
                div {
                    h2 { class: "dashboard-panel-title", style: "margin: 0;", "Organization Members ({members.len()})" }
                    p { style: "margin: 4px 0 0; color: var(--text-muted); font-size: var(--font-size-body-sm);",
                        "Current members in {organization_name}. For full table filtering and access rule configuration, visit Role Management."
                    }
                }
                div { style: "display: flex; gap: 8px;",
                    Link {
                        to: Route::OrganizationRolesPage { organization_id: org_id },
                        class: "btn-action-sm",
                        style: "text-decoration: none;",
                        "🏷 Open Role Management & Filters →"
                    }
                }
            }

            div { style: "display: flex; flex-direction: column; gap: 12px;",
                for member in members {
                    {
                        let is_gov = member.user_id == current_gov_id;
                        let member_id = member.user_id;
                        rsx! {
                            GovernorMemberRow {
                                key: "{member_id}",
                                member,
                                is_gov,
                                on_assign_role: move |m| on_assign_role.call(m),
                                on_remove_member: move |m| on_remove_member.call(m),
                                on_revoke_role: move |args| on_revoke_role.call(args),
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Single member row with assigned roles and administrative actions.
#[component]
fn GovernorMemberRow(
    member: OrganizationMember,
    is_gov: bool,
    on_assign_role: EventHandler<OrganizationMember>,
    on_remove_member: EventHandler<OrganizationMember>,
    on_revoke_role: EventHandler<(i64, String)>,
) -> Element {
    let uid = member.user_id;
    let m_for_assign = member.clone();
    let m_for_remove = member.clone();

    rsx! {
        div { class: "inbox-item",
            div { class: "inbox-item-header",
                div { style: "display: flex; gap: 8px; align-items: center;",
                    span { class: "inbox-item-title", "{member.user_name}" }
                    span { class: "inbox-item-meta", "{member.user_email}" }
                    if is_gov {
                        span { class: "task-detail-status status-accepted badge-sm", "Governor" }
                    } else {
                        span { class: "task-detail-status status-suggested badge-sm", "Base: {member.role}" }
                    }
                }
                div { style: "display: flex; gap: 8px; align-items: center;",
                    button {
                        r#type: "button",
                        class: "btn-action-sm",
                        onclick: move |_| on_assign_role.call(m_for_assign.clone()),
                        "+ Assign Role"
                    }
                    if !is_gov {
                        button {
                            r#type: "button",
                            class: "btn-action-sm",
                            style: "color: var(--danger); border-color: var(--danger);",
                            onclick: move |_| on_remove_member.call(m_for_remove.clone()),
                            "Remove"
                        }
                    }
                }
            }

            div { style: "display: flex; gap: 6px; flex-wrap: wrap; align-items: center; margin-top: 4px;",
                span { style: "font-size: var(--font-size-meta); color: var(--text-muted);", "Assigned Roles:" }
                if member.roles.is_empty() {
                    span { style: "font-size: var(--font-size-meta); color: var(--text-muted); font-style: italic;",
                        "None (base membership only)"
                    }
                } else {
                    for r in &member.roles {
                        {
                            let r_name = r.clone();
                            let r_name_click = r.clone();
                            rsx! {
                                span {
                                    key: "{r_name}",
                                    style: "display: inline-flex; align-items: center; gap: 6px; background-color: var(--primary-bg); color: var(--primary); border: 1px solid var(--primary); border-radius: var(--radius-pill); padding: 2px 8px; font-size: var(--font-size-meta); font-weight: var(--font-weight-bold);",
                                    "{r_name}"
                                    button {
                                        r#type: "button",
                                        style: "background: none; border: none; cursor: pointer; color: var(--primary); font-size: 14px; line-height: 1; padding: 0;",
                                        title: "Revoke role {r_name}",
                                        onclick: move |_| on_revoke_role.call((uid, r_name_click.clone())),
                                        "×"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Danger zone panel for transfer of governance or permanent deletion.
#[component]
fn GovernorDangerZoneSection(
    on_transfer: EventHandler<()>,
    on_delete: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "dashboard-panel", style: "border: 1px solid var(--danger-bg);",
            h2 { class: "dashboard-panel-title", style: "color: var(--danger);", "Governance & Danger Zone" }
            div { style: "display: flex; flex-direction: column; gap: 16px;",
                div { style: "display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 12px; padding: 12px; background: var(--bg-surface-raised); border-radius: var(--radius-sm);",
                    div {
                        strong { style: "display: block; color: var(--text-primary);", "Transfer Governance" }
                        span { style: "font-size: var(--font-size-meta); color: var(--text-muted);",
                            "Nominate another member to become the organization governor. You step down once they accept."
                        }
                    }
                    button {
                        r#type: "button",
                        class: "btn-action-sm",
                        onclick: move |_| on_transfer.call(()),
                        "Transfer Governance"
                    }
                }

                div { style: "display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 12px; padding: 12px; background: var(--bg-surface-raised); border-radius: var(--radius-sm); border-left: 3px solid var(--danger);",
                    div {
                        strong { style: "display: block; color: var(--danger);", "Delete Organization" }
                        span { style: "font-size: var(--font-size-meta); color: var(--text-muted);",
                            "Permanently delete this organization, all its roles, and rules. Requires all other members to have left or been removed first."
                        }
                    }
                    button {
                        r#type: "button",
                        class: "btn-action-sm",
                        style: "color: var(--danger); border-color: var(--danger); font-weight: var(--font-weight-bold);",
                        onclick: move |_| on_delete.call(()),
                        "Delete Organization"
                    }
                }
            }
        }
    }
}

/// Host dialog container managing open modals for governor administrative actions.
#[component]
fn GovernorModalsHost(
    org_id: i64,
    org_name: String,
    current_gov_id: i64,
    roles: Vec<String>,
    all_users: Vec<User>,
    members: Vec<OrganizationMember>,
    member_to_remove: Option<OrganizationMember>,
    is_removing_member: bool,
    pending_warning: Option<(String, PendingWarningAction)>,
    is_warning_loading: bool,
    show_invite_modal: bool,
    show_create_project_modal: bool,
    show_create_role_modal: bool,
    active_assign_member: Option<OrganizationMember>,
    show_transfer_modal: bool,
    show_delete_modal: bool,
    is_deleting: bool,
    delete_error: Option<String>,
    on_close_remove_member: EventHandler<()>,
    on_confirm_remove_member: EventHandler<()>,
    on_close_warning: EventHandler<()>,
    on_confirm_warning: EventHandler<()>,
    on_close_invite: EventHandler<()>,
    on_close_create_project: EventHandler<()>,
    on_close_create_role: EventHandler<()>,
    on_close_assign: EventHandler<()>,
    on_close_transfer: EventHandler<()>,
    on_close_delete: EventHandler<()>,
    on_confirm_delete: EventHandler<String>,
    on_refresh: EventHandler<()>,
) -> Element {
    rsx! {
        if let Some(m) = member_to_remove {
            ConfirmRemoveMemberModal {
                member_name: m.user_name.clone(),
                on_close: move |_| on_close_remove_member.call(()),
                on_confirm: move |_| on_confirm_remove_member.call(()),
                is_loading: is_removing_member,
            }
        }

        if let Some((msg, _)) = pending_warning {
            ConfirmActiveTasksWarningModal {
                warning_message: msg,
                on_close: move |_| on_close_warning.call(()),
                on_confirm: move |_| on_confirm_warning.call(()),
                is_loading: is_warning_loading,
            }
        }

        if show_invite_modal {
            InviteOrgMemberModal {
                organization_id: org_id,
                all_users: all_users.clone(),
                available_roles: roles.clone(),
                on_close: move |_| on_close_invite.call(()),
                on_invited: move |_| {
                    on_close_invite.call(());
                    on_refresh.call(());
                },
            }
        }

        if show_create_project_modal {
            CreateOrgProjectModal {
                organization_id: org_id,
                organization_name: org_name.clone(),
                on_close: move |_| on_close_create_project.call(()),
                on_created: move |_| {
                    on_close_create_project.call(());
                    on_refresh.call(());
                },
            }
        }

        if show_create_role_modal {
            CreateOrgRoleModal {
                organization_id: org_id,
                on_close: move |_| on_close_create_role.call(()),
                on_created: move |_| {
                    on_close_create_role.call(());
                    on_refresh.call(());
                },
            }
        }

        if let Some(m) = active_assign_member {
            AssignMemberRoleModal {
                organization_id: org_id,
                member: m,
                available_roles: roles.clone(),
                on_close: move |_| on_close_assign.call(()),
                on_assigned: move |_| {
                    on_close_assign.call(());
                    on_refresh.call(());
                },
            }
        }

        if show_transfer_modal {
            TransferOrgGovernanceModal {
                organization_id: org_id,
                members: members.clone(),
                current_governor_id: current_gov_id,
                on_close: move |_| on_close_transfer.call(()),
                on_transferred: move |_| {
                    on_close_transfer.call(());
                    on_refresh.call(());
                },
            }
        }

        if show_delete_modal {
            ConfirmDeleteModal {
                entity_type: "organization".to_string(),
                entity_name: org_name,
                on_close: move |_| on_close_delete.call(()),
                on_confirm: move |confirmation| on_confirm_delete.call(confirmation),
                is_deleting,
                error: delete_error,
            }
        }
    }
}
