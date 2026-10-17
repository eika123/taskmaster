use dioxus::prelude::*;
use crate::Route;
use crate::types::OrganizationMember;
use crate::backend::app_endpoints::{
    get_organization, get_organization_affiliated_projects, get_organization_members,
    get_organization_role_rules, get_organization_roles, remove_organization_member,
    revoke_organization_member_role, revoke_org_role_project_access,
    set_org_role_project_access,
};
use super::modals::{
    AssignMemberRoleModal, ConfirmActiveTasksWarningModal, CreateOrgRoleModal,
};

#[derive(Clone, PartialEq, Eq)]
enum PendingWarningAction {
    RevokeRole { user_id: i64, role: String },
    RevokeRule { project_id: i64, org_role: String },
}

#[component]
pub fn OrganizationRolesPage(organization_id: i64) -> Element {
    let org_resource = use_server_future(move || async move {
        get_organization(organization_id).await
    })?;

    let members_resource = use_server_future(move || async move {
        get_organization_members(organization_id).await
    })?;

    let roles_resource = use_server_future(move || async move {
        get_organization_roles(organization_id).await
    })?;

    let projects_resource = use_server_future(move || async move {
        get_organization_affiliated_projects(organization_id).await
    })?;

    let rules_resource = use_server_future(move || async move {
        get_organization_role_rules(organization_id).await
    })?;

    let auth = use_context::<crate::components::AuthState>();
    let current_user_id = auth.user.read().as_ref().map(|u| u.id).unwrap_or(0);

    let organization = match &*org_resource.read() {
        Some(Ok(org)) => org.clone(),
        Some(Err(err)) => {
            return rsx! {
                div { class: "dashboard-container",
                    div { class: "task-detail-error", "Failed to load organization: {err}" }
                }
            };
        }
        None => {
            return rsx! {
                div { class: "dashboard-container", p { "Loading..." } }
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

    let role_rules = match &*rules_resource.read() {
        Some(Ok(r)) => r.clone(),
        _ => Vec::new(),
    };

    let is_governor = organization.governing_user_id == current_user_id;

    if !is_governor {
        return rsx! {
            div { class: "dashboard-container",
                div { class: "dashboard-panel",
                    h2 { class: "dashboard-panel-title", style: "color: var(--danger);", "Access Restricted" }
                    p { "Only the organization governor has access to configure roles and access rules." }
                    Link {
                        to: Route::OrganizationDashboardPage { organization_id },
                        class: "btn-action-sm",
                        "← Return to Organization Dashboard"
                    }
                }
            }
        };
    }

    // Interactive filters for member roles table
    let mut search_query = use_signal(String::new);
    let mut role_filter = use_signal(|| Option::<String>::None);

    // Modals state
    let mut show_create_role = use_signal(|| false);
    let mut active_assign_member = use_signal(|| Option::<OrganizationMember>::None);

    // Warning confirmation modal state for active tasks
    let mut pending_warning = use_signal(|| Option::<(String, PendingWarningAction)>::None);
    let mut is_warning_loading = use_signal(|| false);

    // New rule state
    let mut new_rule_project_id = use_signal(|| affiliated_projects.first().map(|p| p.project_id));
    let mut new_rule_org_role = use_signal(|| roles.first().cloned().unwrap_or_else(|| "member".to_string()));
    let mut new_rule_project_role = use_signal(|| "member".to_string());
    let mut rule_error = use_signal(|| Option::<String>::None);
    let mut is_adding_rule = use_signal(|| false);

    let handle_refresh = move || {
        let mut m = members_resource;
        let mut ro = roles_resource;
        let mut p = projects_resource;
        let mut ru = rules_resource;
        m.restart();
        ro.restart();
        p.restart();
        ru.restart();
    };

    let handle_revoke_role = move |(uid, role_name): (i64, String)| {
        let r_name_clone = role_name.clone();
        spawn(async move {
            match revoke_organization_member_role(organization_id, uid, role_name.clone(), false).await {
                Ok(_) => handle_refresh(),
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

    let handle_remove_member = move |uid: i64| {
        spawn(async move {
            if remove_organization_member(organization_id, uid).await.is_ok() {
                handle_refresh();
            }
        });
    };

    let handle_revoke_rule = move |(pid, o_role): (i64, String)| {
        let o_role_clone = o_role.clone();
        spawn(async move {
            match revoke_org_role_project_access(pid, organization_id, o_role.clone(), false).await {
                Ok(_) => handle_refresh(),
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
                        let _ = revoke_organization_member_role(organization_id, user_id, role, true).await;
                    }
                    PendingWarningAction::RevokeRule { project_id, org_role } => {
                        let _ = revoke_org_role_project_access(project_id, organization_id, org_role, true).await;
                    }
                }
                is_warning_loading.set(false);
                pending_warning.set(None);
                handle_refresh();
            });
        }
    };

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
            match set_org_role_project_access(pid, organization_id, o_role, p_role).await {
                Ok(_) => {
                    is_adding_rule.set(false);
                    handle_refresh();
                }
                Err(e) => {
                    is_adding_rule.set(false);
                    rule_error.set(Some(e.to_string()));
                }
            }
        });
    };

    // Filtered members calculation
    let query_val = search_query.read().trim().to_lowercase();
    let selected_role = role_filter.read().clone();

    let filtered_members: Vec<OrganizationMember> = members
        .iter()
        .filter(|m| {
            let matches_query = query_val.is_empty()
                || m.user_name.to_lowercase().contains(&query_val)
                || m.user_email.to_lowercase().contains(&query_val);

            let matches_role = match &selected_role {
                None => true,
                Some(r) => m.roles.contains(r) || m.role == *r,
            };

            matches_query && matches_role
        })
        .cloned()
        .collect();

    rsx! {
        div { class: "dashboard-container",
            // Navigation Bar
            div { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 20px;",
                Link {
                    to: Route::OrganizationDashboardPage { organization_id },
                    style: "color: var(--primary); text-decoration: none; font-size: var(--font-size-body-sm); font-weight: var(--font-weight-bold); display: inline-flex; align-items: center; gap: 6px;",
                    "← Back to {organization.name} Dashboard"
                }
                span { class: "task-detail-status status-accepted badge-sm", "Governor Privileges" }
            }

            div { style: "display: flex; flex-direction: column; gap: 24px;",
                // Header
                div { class: "dashboard-panel",
                    div { style: "display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 12px;",
                        div {
                            h1 { class: "dashboard-title", style: "margin: 0;", "Role Management & Access Rules" }
                            p { style: "margin: 4px 0 0; color: var(--text-secondary); font-size: var(--font-size-body-sm);",
                                "Configure custom functional roles, multi-role assignments, and project access rules for {organization.name}."
                            }
                        }
                        button {
                            r#type: "button",
                            class: "btn-task-edit",
                            onclick: move |_| show_create_role.set(true),
                            "+ Define New Role"
                        }
                    }
                }

                // Section 1: Member Roles Table with Live Filter
                div { class: "dashboard-panel",
                    div { style: "display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 12px;",
                        div {
                            h2 { class: "dashboard-panel-title", style: "margin: 0;", "Member Role Assignments" }
                            p { style: "margin: 4px 0 0; color: var(--text-muted); font-size: var(--font-size-body-sm);",
                                "Filter members by name or role to quickly inspect and adjust their assigned responsibilities."
                            }
                        }
                        span { style: "font-size: var(--font-size-meta); color: var(--text-muted);",
                            "Showing {filtered_members.len()} of {members.len()} members"
                        }
                    }

                    // Filter toolbar
                    div { style: "display: flex; gap: 12px; flex-wrap: wrap; align-items: center; background: var(--bg-surface-raised); padding: 12px; border-radius: var(--radius-sm); border: 1px solid var(--border-subtle);",
                        div { style: "flex: 1; min-width: 220px;",
                            input {
                                class: "task-edit-title-input",
                                placeholder: "🔍 Search members by name or email...",
                                value: "{search_query}",
                                oninput: move |e| search_query.set(e.value()),
                            }
                        }
                        div { style: "display: flex; gap: 6px; align-items: center; flex-wrap: wrap;",
                            span { style: "font-size: var(--font-size-meta); color: var(--text-muted); font-weight: var(--font-weight-bold);", "Role Filter:" }
                            button {
                                r#type: "button",
                                class: if role_filter.read().is_none() { "column-pill pill-active" } else { "column-pill" },
                                onclick: move |_| role_filter.set(None),
                                "All Roles"
                            }
                            for r in &roles {
                                {
                                    let role_name = r.clone();
                                    let is_active = *role_filter.read() == Some(role_name.clone());
                                    let r_click = role_name.clone();
                                    rsx! {
                                        button {
                                            key: "{role_name}",
                                            r#type: "button",
                                            class: if is_active { "column-pill pill-active" } else { "column-pill" },
                                            onclick: move |_| role_filter.set(Some(r_click.clone())),
                                            "{role_name}"
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Formatted Members Table
                    if filtered_members.is_empty() {
                        p { class: "empty-description", "No members match the current filter criteria." }
                    } else {
                        div { style: "overflow-x: auto;",
                            table { style: "width: 100%; border-collapse: collapse; text-align: left; font-size: var(--font-size-body-sm);",
                                thead {
                                    tr { style: "border-bottom: 2px solid var(--border-default); color: var(--text-muted); font-size: var(--font-size-meta); text-transform: uppercase;",
                                        th { style: "padding: 10px 12px;", "Member" }
                                        th { style: "padding: 10px 12px;", "Base Role" }
                                        th { style: "padding: 10px 12px;", "Assigned Roles (Multi-Role)" }
                                        th { style: "padding: 10px 12px; text-align: right;", "Actions" }
                                    }
                                }
                                tbody {
                                    for member in filtered_members.iter() {
                                        {
                                            let m = member.clone();
                                            let uid = m.user_id;
                                            let is_gov = uid == organization.governing_user_id;
                                            let m_for_assign = m.clone();
                                            rsx! {
                                                tr { key: "{uid}", style: "border-bottom: 1px solid var(--border-subtle);",
                                                    td { style: "padding: 12px;",
                                                        strong { style: "display: block; color: var(--text-primary);", "{m.user_name}" }
                                                        span { style: "color: var(--text-muted); font-size: var(--font-size-meta);", "{m.user_email}" }
                                                    }
                                                    td { style: "padding: 12px;",
                                                        if is_gov {
                                                            span { class: "task-detail-status status-accepted badge-sm", "Governor" }
                                                        } else {
                                                            span { class: "task-detail-status status-suggested badge-sm", "{m.role}" }
                                                        }
                                                    }
                                                    td { style: "padding: 12px;",
                                                        div { style: "display: flex; gap: 6px; flex-wrap: wrap; align-items: center;",
                                                            if m.roles.is_empty() {
                                                                span { style: "color: var(--text-muted); font-style: italic; font-size: var(--font-size-meta);", "None" }
                                                            } else {
                                                                for r in m.roles.iter() {
                                                                    {
                                                                        let r_name = r.clone();
                                                                        let r_click = r.clone();
                                                                        rsx! {
                                                                            span {
                                                                                key: "{r_name}",
                                                                                style: "display: inline-flex; align-items: center; gap: 6px; background-color: var(--primary-bg); color: var(--primary); border: 1px solid var(--primary); border-radius: var(--radius-pill); padding: 2px 8px; font-size: var(--font-size-meta); font-weight: var(--font-weight-bold);",
                                                                                "{r_name}"
                                                                                button {
                                                                                    r#type: "button",
                                                                                    style: "background: none; border: none; cursor: pointer; color: var(--primary); font-size: 14px; line-height: 1; padding: 0;",
                                                                                    title: "Revoke role {r_name}",
                                                                                    onclick: move |_| handle_revoke_role((uid, r_click.clone())),
                                                                                    "×"
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                    td { style: "padding: 12px; text-align: right;",
                                                        div { style: "display: inline-flex; gap: 8px;",
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
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Section 2: Defined Custom Roles
                div { class: "dashboard-panel",
                    div { style: "display: flex; justify-content: space-between; align-items: center;",
                        div {
                            h2 { class: "dashboard-panel-title", style: "margin: 0;", "Custom Functional Roles ({roles.len()})" }
                            p { style: "margin: 4px 0 0; color: var(--text-muted); font-size: var(--font-size-body-sm);",
                                "Specialized roles created in this organization. Members can have any combination of these."
                            }
                        }
                        button {
                            r#type: "button",
                            class: "btn-action-sm",
                            onclick: move |_| show_create_role.set(true),
                            "+ New Role"
                        }
                    }

                    div { style: "display: flex; gap: 10px; flex-wrap: wrap; margin-top: 8px;",
                        for r in &roles {
                            {
                                let count = members.iter().filter(|m| m.roles.contains(r)).count();
                                rsx! {
                                    div {
                                        key: "{r}",
                                        style: "display: flex; align-items: center; gap: 8px; background-color: var(--bg-surface-raised); border: 1px solid var(--border-default); border-radius: var(--radius-sm); padding: 8px 14px;",
                                        strong { style: "color: var(--text-primary);", "🏷 {r}" }
                                        span { style: "background-color: var(--primary-bg); color: var(--primary); padding: 2px 6px; border-radius: var(--radius-pill); font-size: var(--font-size-meta); font-weight: var(--font-weight-bold);",
                                            "{count} members"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Section 3: Role-to-Project Access Rules
                div { class: "dashboard-panel",
                    div {
                        h2 { class: "dashboard-panel-title", "Role-to-Project Access Rules ({role_rules.len()})" }
                        p { style: "margin: 4px 0 0; color: var(--text-muted); font-size: var(--font-size-body-sm);",
                            "Rules define what project roles members automatically inherit based on their organization roles. The granted role is always capped by the project's max privilege ceiling."
                        }
                    }

                    if role_rules.is_empty() {
                        p { class: "empty-description", "No project access rules configured yet. Create one below!" }
                    } else {
                        div { style: "display: flex; flex-direction: column; gap: 10px;",
                            for rule in role_rules.iter() {
                                {
                                    let pid = rule.project_id;
                                    let o_role = rule.org_role.clone();
                                    let o_role_del = rule.org_role.clone();
                                    rsx! {
                                        div { key: "{pid}-{o_role}", class: "inbox-item",
                                            div { style: "display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 10px;",
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
                                                div { style: "display: flex; gap: 8px; align-items: center;",
                                                    Link {
                                                        to: Route::ProjectManagePage { project_id: pid },
                                                        class: "btn-action-sm",
                                                        style: "text-decoration: none;",
                                                        "⚙ Project Access"
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
                    }

                    // Add new access rule form
                    if !affiliated_projects.is_empty() && !roles.is_empty() {
                        div { style: "border-top: 1px solid var(--border-subtle); padding-top: 16px; margin-top: 10px;",
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
        if *show_create_role.read() {
            CreateOrgRoleModal {
                organization_id,
                on_close: move |_| show_create_role.set(false),
                on_created: move |_| {
                    show_create_role.set(false);
                    handle_refresh();
                },
            }
        }

        if let Some(m) = active_assign_member.read().clone() {
            AssignMemberRoleModal {
                organization_id,
                member: m,
                available_roles: roles.clone(),
                on_close: move |_| active_assign_member.set(None),
                on_assigned: move |_| {
                    active_assign_member.set(None);
                    handle_refresh();
                },
            }
        }
    }
}
