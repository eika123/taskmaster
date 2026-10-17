use dioxus::prelude::*;
use crate::Route;
use crate::types::{
    Organization, OrganizationMember, OrgProjectAffiliation, OrgRoleRuleItem, User,
};
use crate::backend::app_endpoints::{
    delete_organization, remove_organization_member,
    revoke_organization_member_role, revoke_org_role_project_access,
    set_org_role_project_access,
};
use super::modals::{
    AssignMemberRoleModal, ConfirmActiveTasksWarningModal, CreateOrgRoleModal,
    InviteOrgMemberModal, TransferOrgGovernanceModal,
};
use crate::components::dashboard::ConfirmDeleteModal;

#[component]
pub fn OrganizationGovernorView(
    organization: Organization,
    members: Vec<OrganizationMember>,
    roles: Vec<String>,
    affiliated_projects: Vec<OrgProjectAffiliation>,
    role_rules: Vec<OrgRoleRuleItem>,
    all_users: Vec<User>,
    on_refresh: EventHandler<()>,
) -> Element {
    let org_id = organization.id;
    let org_name = organization.name.clone();
    let current_gov_id = organization.governing_user_id;

#[derive(Clone, PartialEq, Eq)]
enum PendingWarningAction {
    RevokeRole { user_id: i64, role: String },
    RevokeRule { project_id: i64, org_role: String },
}

    // Modals state
    let mut show_invite_modal = use_signal(|| false);
    let mut show_create_role_modal = use_signal(|| false);
    let mut show_transfer_modal = use_signal(|| false);
    let mut show_delete_modal = use_signal(|| false);
    let mut active_assign_member = use_signal(|| Option::<OrganizationMember>::None);

    // Warning confirmation modal state for active tasks
    let mut pending_warning = use_signal(|| Option::<(String, PendingWarningAction)>::None);
    let mut is_warning_loading = use_signal(|| false);

    // New Project Access Rule form state
    let mut new_rule_project_id = use_signal(|| {
        affiliated_projects.first().map(|p| p.project_id)
    });
    let mut new_rule_org_role = use_signal(|| {
        roles.first().cloned().unwrap_or_else(|| "member".to_string())
    });
    let mut new_rule_project_role = use_signal(|| "member".to_string());
    let mut rule_error = use_signal(|| Option::<String>::None);
    let mut is_adding_rule = use_signal(|| false);

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
    let handle_remove_member = move |uid: i64| {
        spawn(async move {
            if remove_organization_member(org_id, uid).await.is_ok() {
                on_refresh.call(());
            }
        });
    };

    // Handler to revoke project role rule
    let handle_revoke_rule = move |(pid, o_role): (i64, String)| {
        let o_role_clone = o_role.clone();
        spawn(async move {
            match revoke_org_role_project_access(pid, org_id, o_role.clone(), false).await {
                Ok(_) => on_refresh.call(()),
                Err(e) => {
                    let err_str = e.to_string();
                    if err_str.contains("Active task impact warning") {
                        pending_warning.set(Some((
                            err_str,
                            PendingWarningAction::RevokeRule {
                                project_id: pid,
                                org_role: o_role_clone,
                            },
                        )));
                    }
                }
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
                    PendingWarningAction::RevokeRule { project_id, org_role } => {
                        let _ = revoke_org_role_project_access(project_id, org_id, org_role, true).await;
                    }
                }
                is_warning_loading.set(false);
                pending_warning.set(None);
                on_refresh.call(());
            });
        }
    };

    // Handler to add new project access rule
    let handle_add_rule = move |_| {
        let pid = match *new_rule_project_id.read() {
            Some(id) => id,
            None => {
                rule_error.set(Some("Please select an affiliated project.".to_string()));
                return;
            }
        };

        let o_role = new_rule_org_role.read().clone();
        let p_role = new_rule_project_role.read().clone();

        is_adding_rule.set(true);
        rule_error.set(None);

        spawn(async move {
            match set_org_role_project_access(pid, org_id, o_role, p_role).await {
                Ok(_) => {
                    is_adding_rule.set(false);
                    on_refresh.call(());
                }
                Err(e) => {
                    is_adding_rule.set(false);
                    rule_error.set(Some(e.to_string()));
                }
            }
        });
    };

    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 24px;",
            // Header Card
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
                        button {
                            r#type: "button",
                            class: "btn-task-edit",
                            onclick: move |_| show_invite_modal.set(true),
                            "👥 + Invite Member"
                        }
                        button {
                            r#type: "button",
                            class: "btn-task-cancel",
                            onclick: move |_| show_create_role_modal.set(true),
                            "🏷 + New Custom Role"
                        }
                    }
                }

                // Stats row
                div { style: "display: grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); gap: 12px; margin-top: 8px;",
                    div { style: "background-color: var(--bg-surface-raised); border: 1px solid var(--border-subtle); border-radius: var(--radius-sm); padding: 12px;",
                        div { style: "font-size: var(--font-size-meta); color: var(--text-muted); text-transform: uppercase; font-weight: var(--font-weight-bold);", "Total Members" }
                        div { style: "font-size: var(--font-size-containerheader-medium); font-weight: var(--font-weight-bold); color: var(--text-primary); margin-top: 4px;", "{members.len()}" }
                    }
                    div { style: "background-color: var(--bg-surface-raised); border: 1px solid var(--border-subtle); border-radius: var(--radius-sm); padding: 12px;",
                        div { style: "font-size: var(--font-size-meta); color: var(--text-muted); text-transform: uppercase; font-weight: var(--font-weight-bold);", "Defined Roles" }
                        div { style: "font-size: var(--font-size-containerheader-medium); font-weight: var(--font-weight-bold); color: var(--text-primary); margin-top: 4px;", "{roles.len()}" }
                    }
                    div { style: "background-color: var(--bg-surface-raised); border: 1px solid var(--border-subtle); border-radius: var(--radius-sm); padding: 12px;",
                        div { style: "font-size: var(--font-size-meta); color: var(--text-muted); text-transform: uppercase; font-weight: var(--font-weight-bold);", "Affiliated Projects" }
                        div { style: "font-size: var(--font-size-containerheader-medium); font-weight: var(--font-weight-bold); color: var(--text-primary); margin-top: 4px;", "{affiliated_projects.len()}" }
                    }
                    div { style: "background-color: var(--bg-surface-raised); border: 1px solid var(--border-subtle); border-radius: var(--radius-sm); padding: 12px;",
                        div { style: "font-size: var(--font-size-meta); color: var(--text-muted); text-transform: uppercase; font-weight: var(--font-weight-bold);", "Role Access Rules" }
                        div { style: "font-size: var(--font-size-containerheader-medium); font-weight: var(--font-weight-bold); color: var(--text-primary); margin-top: 4px;", "{role_rules.len()}" }
                    }
                }
            }

            // Section 1: Member Directory & Multi-Role Assignment
            div { class: "dashboard-panel",
                div { style: "display: flex; justify-content: space-between; align-items: center;",
                    div {
                        h2 { class: "dashboard-panel-title", style: "margin: 0;", "Organization Members ({members.len()})" }
                        p { style: "margin: 4px 0 0; color: var(--text-muted); font-size: var(--font-size-body-sm);",
                            "Assign multiple functional roles to members so they can participate in corresponding project activities."
                        }
                    }
                }

                div { style: "display: flex; flex-direction: column; gap: 12px;",
                    for member in members.iter() {
                        {
                            let m = member.clone();
                            let is_gov = m.user_id == current_gov_id;
                            let uid = m.user_id;
                            let m_for_assign = m.clone();
                            rsx! {
                                div { key: "{uid}", class: "inbox-item",
                                    div { class: "inbox-item-header",
                                        div { style: "display: flex; gap: 8px; align-items: center;",
                                            span { class: "inbox-item-title", "{m.user_name}" }
                                            span { class: "inbox-item-meta", "{m.user_email}" }
                                            if is_gov {
                                                span { class: "task-detail-status status-accepted badge-sm", "Governor" }
                                            } else {
                                                span { class: "task-detail-status status-suggested badge-sm", "Base: {m.role}" }
                                            }
                                        }
                                        div { style: "display: flex; gap: 8px; align-items: center;",
                                            button {
                                                r#type: "button",
                                                class: "btn-action-sm",
                                                onclick: move |_| active_assign_member.set(Some(m_for_assign.clone())),
                                                "+ Assign Role"
                                            }
                                            if !is_gov {
                                                button {
                                                    r#type: "button",
                                                    class: "btn-action-sm",
                                                    style: "color: var(--danger); border-color: var(--danger);",
                                                    onclick: move |_| handle_remove_member(uid),
                                                    "Remove"
                                                }
                                            }
                                        }
                                    }

                                    // Role Badges for this user
                                    div { style: "display: flex; gap: 6px; flex-wrap: wrap; align-items: center; margin-top: 4px;",
                                        span { style: "font-size: var(--font-size-meta); color: var(--text-muted);", "Assigned Roles:" }
                                        if m.roles.is_empty() {
                                            span { style: "font-size: var(--font-size-meta); color: var(--text-muted); font-style: italic;",
                                                "None (base membership only)"
                                            }
                                        } else {
                                            for r in m.roles.iter() {
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
                                                                onclick: move |_| handle_revoke_member_role((uid, r_name_click.clone())),
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
                    }
                }
            }

            // Section 2: Defined Organization Roles
            div { class: "dashboard-panel",
                div { style: "display: flex; justify-content: space-between; align-items: center;",
                    div {
                        h2 { class: "dashboard-panel-title", style: "margin: 0;", "Custom Functional Roles ({roles.len()})" }
                        p { style: "margin: 4px 0 0; color: var(--text-muted); font-size: var(--font-size-body-sm);",
                            "Define organizational specializations like engineering, support, marketing, or compliance."
                        }
                    }
                    button {
                        r#type: "button",
                        class: "btn-action-sm",
                        onclick: move |_| show_create_role_modal.set(true),
                        "+ Define New Role"
                    }
                }

                div { style: "display: flex; gap: 8px; flex-wrap: wrap; margin-top: 8px;",
                    for r in &roles {
                        span {
                            key: "{r}",
                            style: "background-color: var(--bg-surface-raised); border: 1px solid var(--border-default); border-radius: var(--radius-sm); padding: 6px 12px; font-weight: var(--font-weight-bold); font-size: var(--font-size-body-sm); color: var(--text-primary);",
                            "🏷 {r}"
                        }
                    }
                }
            }

            // Section 3: Project Role Access Rules
            div { class: "dashboard-panel",
                div {
                    h2 { class: "dashboard-panel-title", "Role-to-Project Access Rules ({role_rules.len()})" }
                    p { style: "margin: 4px 0 0; color: var(--text-muted); font-size: var(--font-size-body-sm);",
                        "Map organizational roles to access levels in affiliated projects. Members with matching roles automatically inherit these grants (subject to project privilege ceilings)."
                    }
                }

                // Existing rules list
                if role_rules.is_empty() {
                    p { class: "empty-description", "No project access rules configured yet. Define one below!" }
                } else {
                    div { style: "display: flex; flex-direction: column; gap: 8px;",
                        for rule in role_rules.iter() {
                            {
                                let pid = rule.project_id;
                                let o_role = rule.org_role.clone();
                                let o_role_del = rule.org_role.clone();
                                rsx! {
                                    div { key: "{pid}-{o_role}", class: "inbox-item",
                                        div { style: "display: flex; justify-content: space-between; align-items: center;",
                                            div { style: "display: flex; gap: 10px; align-items: center; flex-wrap: wrap;",
                                                span { class: "inbox-item-title", "{rule.project_name}" }
                                                span { style: "color: var(--text-muted); font-size: var(--font-size-meta);", "Org Role:" }
                                                span { style: "background-color: var(--primary-bg); color: var(--primary); padding: 2px 8px; border-radius: var(--radius-sm); font-weight: var(--font-weight-bold); font-size: var(--font-size-meta);",
                                                    "{rule.org_role}"
                                                }
                                                span { style: "color: var(--text-muted); font-size: var(--font-size-meta);", "➔ Project Role:" }
                                                span { class: "task-detail-status status-accepted badge-sm",
                                                    "{rule.project_role}"
                                                }
                                            }
                                            button {
                                                r#type: "button",
                                                class: "btn-action-sm",
                                                style: "color: var(--danger); border-color: var(--danger);",
                                                onclick: move |_| handle_revoke_rule((pid, o_role_del.clone())),
                                                "Revoke Rule"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Add new access rule form
                if !affiliated_projects.is_empty() && !roles.is_empty() {
                    div { style: "border-top: 1px solid var(--border-subtle); padding-top: 16px; margin-top: 8px;",
                        h3 { style: "margin: 0 0 12px 0; font-size: var(--font-size-cardheader-small); color: var(--text-primary);",
                            "+ Add Project Access Rule"
                        }
                        if let Some(err) = &*rule_error.read() {
                            div { class: "task-detail-error", style: "margin-bottom: 12px;", "{err}" }
                        }
                        div { style: "display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 12px; align-items: flex-end;",
                            div {
                                label { class: "form-label", "Affiliated Project" }
                                select {
                                    class: "task-status-select",
                                    onchange: move |e| {
                                        if let Ok(id) = e.value().parse::<i64>() {
                                            new_rule_project_id.set(Some(id));
                                        }
                                    },
                                    for p in &affiliated_projects {
                                        option { value: "{p.project_id}", "{p.project_name} (Max: {p.max_granted_role})" }
                                    }
                                }
                            }
                            div {
                                label { class: "form-label", "Organization Role" }
                                select {
                                    class: "task-status-select",
                                    value: "{new_rule_org_role}",
                                    onchange: move |e| new_rule_org_role.set(e.value()),
                                    for r in &roles {
                                        option { value: "{r}", "{r}" }
                                    }
                                }
                            }
                            div {
                                label { class: "form-label", "Project Role to Grant" }
                                select {
                                    class: "task-status-select",
                                    value: "{new_rule_project_role}",
                                    onchange: move |e| new_rule_project_role.set(e.value()),
                                    option { value: "viewer", "Viewer (Read Only)" }
                                    option { value: "member", "Member (Create & Edit Tasks)" }
                                    option { value: "admin", "Admin (Manage Settings & Delete)" }
                                }
                            }
                            button {
                                r#type: "button",
                                class: "btn-task-save",
                                disabled: *is_adding_rule.read(),
                                onclick: handle_add_rule,
                                if *is_adding_rule.read() { "Granting..." } else { "Grant Access Rule" }
                            }
                        }
                    }
                } else if affiliated_projects.is_empty() {
                    p { style: "font-size: var(--font-size-meta); color: var(--text-muted); font-style: italic;",
                        "No affiliated projects yet. Project leaders can affiliate this organization from their project dashboard."
                    }
                }
            }

            // Section 4: Affiliated Projects List
            div { class: "dashboard-panel",
                h2 { class: "dashboard-panel-title", "Affiliated Projects ({affiliated_projects.len()})" }
                if affiliated_projects.is_empty() {
                    p { class: "empty-description", "This organization is not affiliated with any projects yet." }
                } else {
                    div { class: "project-card-list",
                        for p in &affiliated_projects {
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

            // Section 5: Governance & Danger Zone
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
                            onclick: move |_| show_transfer_modal.set(true),
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
                            onclick: move |_| show_delete_modal.set(true),
                            "Delete Organization"
                        }
                    }
                }
            }

            // Active Task Warning confirmation modal
            if let Some((msg, _)) = (*pending_warning.read()).clone() {
                ConfirmActiveTasksWarningModal {
                    warning_message: msg,
                    on_close: move |_| pending_warning.set(None),
                    on_confirm: handle_confirm_warning,
                    is_loading: *is_warning_loading.read(),
                }
            }

            // Modals
            if *show_invite_modal.read() {
                InviteOrgMemberModal {
                    organization_id: org_id,
                    all_users: all_users.clone(),
                    available_roles: roles.clone(),
                    on_close: move |_| show_invite_modal.set(false),
                    on_invited: move |_| {
                        show_invite_modal.set(false);
                        on_refresh.call(());
                    },
                }
            }

            if *show_create_role_modal.read() {
                CreateOrgRoleModal {
                    organization_id: org_id,
                    on_close: move |_| show_create_role_modal.set(false),
                    on_created: move |_| {
                        show_create_role_modal.set(false);
                        on_refresh.call(());
                    },
                }
            }

            if let Some(m) = active_assign_member.read().clone() {
                AssignMemberRoleModal {
                    organization_id: org_id,
                    member: m,
                    available_roles: roles.clone(),
                    on_close: move |_| active_assign_member.set(None),
                    on_assigned: move |_| {
                        active_assign_member.set(None);
                        on_refresh.call(());
                    },
                }
            }

            if *show_transfer_modal.read() {
                TransferOrgGovernanceModal {
                    organization_id: org_id,
                    members: members.clone(),
                    current_governor_id: current_gov_id,
                    on_close: move |_| show_transfer_modal.set(false),
                    on_transferred: move |_| {
                        show_transfer_modal.set(false);
                        on_refresh.call(());
                    },
                }
            }

            if *show_delete_modal.read() {
                ConfirmDeleteModal {
                    entity_type: "organization".to_string(),
                    entity_name: org_name.clone(),
                    on_close: move |_| {
                        show_delete_modal.set(false);
                        delete_error.set(None);
                    },
                    on_confirm: move |confirmation: String| {
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
                    is_deleting: *is_deleting.read(),
                    error: (*delete_error.read()).clone(),
                }
            }
        }
    }
}
