use crate::backend::app_endpoints::{
    affiliate_organization_with_project, delete_project, get_available_organizations_for_project,
    get_project, get_project_members, get_project_organizations, remove_organization_from_project,
};
use crate::components::dashboard::ConfirmDeleteModal;
use crate::components::organizations::modals::ConfirmActiveTasksWarningModal;
use crate::components::projects::InviteProjectMemberModal;
use crate::types::{Organization, Project, ProjectMember, ProjectOrganization};
use crate::Route;
use dioxus::prelude::*;

/// Management dashboard for a project: membership, affiliated organizations, and administrative actions.
#[component]
pub fn ProjectManagePage(project_id: i64) -> Element {
    let proj_resource = use_server_future(move || async move { get_project(project_id).await })?;

    let orgs_resource =
        use_server_future(move || async move { get_project_organizations(project_id).await })?;

    let available_resource = use_server_future(move || async move {
        get_available_organizations_for_project(project_id).await
    })?;

    let members_resource =
        use_server_future(move || async move { get_project_members(project_id).await })?;

    let auth = use_context::<crate::components::AuthState>();
    let current_user_id = auth.user.read().as_ref().map(|u| u.id).unwrap_or(0);
    let nav = use_navigator();

    let project = match &*proj_resource.read() {
        Some(Ok(p)) => p.clone(),
        Some(Err(err)) => {
            return rsx! {
                div { class: "dashboard-container",
                    div { class: "task-detail-error", "Failed to load project: {err}" }
                }
            };
        }
        None => {
            return rsx! {
                div { class: "dashboard-container", p { "Loading project..." } }
            };
        }
    };

    let affiliated_orgs = match &*orgs_resource.read() {
        Some(Ok(orgs)) => orgs.clone(),
        _ => Vec::new(),
    };

    let available_orgs = match &*available_resource.read() {
        Some(Ok(orgs)) => orgs.clone(),
        _ => Vec::new(),
    };

    let project_members = match &*members_resource.read() {
        Some(Ok(m)) => m.clone(),
        _ => Vec::new(),
    };

    let is_leader = project.leader_id == current_user_id;

    let mut form_error = use_signal(|| Option::<String>::None);
    let mut is_affiliating = use_signal(|| false);

    // Warning confirmation modal state for active tasks: (warning_message, org_id)
    let mut pending_warning = use_signal(|| Option::<(String, i64)>::None);
    let mut is_warning_loading = use_signal(|| false);

    // Modals state
    let mut show_invite_modal = use_signal(|| false);
    let mut show_delete_modal = use_signal(|| false);
    let mut is_deleting = use_signal(|| false);
    let mut delete_error = use_signal(|| Option::<String>::None);

    let handle_refresh = move || {
        let mut o = orgs_resource;
        let mut a = available_resource;
        let mut m = members_resource;
        o.restart();
        a.restart();
        m.restart();
    };

    let handle_affiliate_new = move |(oid, rel, max_r): (i64, String, String)| {
        is_affiliating.set(true);
        form_error.set(None);

        spawn(async move {
            match affiliate_organization_with_project(project_id, oid, rel, max_r).await {
                Ok(_) => {
                    is_affiliating.set(false);
                    handle_refresh();
                }
                Err(e) => {
                    is_affiliating.set(false);
                    form_error.set(Some(e.to_string()));
                }
            }
        });
    };

    let handle_remove_org = move |oid: i64| {
        spawn(async move {
            match remove_organization_from_project(project_id, oid, false).await {
                Ok(_) => handle_refresh(),
                Err(e) => {
                    let err_str = e.to_string();
                    if err_str.contains("Active task impact warning") {
                        pending_warning.set(Some((err_str, oid)));
                    } else {
                        form_error.set(Some(err_str));
                    }
                }
            }
        });
    };

    let handle_confirm_delete = move |confirmation: String| {
        is_deleting.set(true);
        delete_error.set(None);
        let nav_clone = nav.clone();
        spawn(async move {
            match delete_project(project_id, confirmation).await {
                Ok(_) => {
                    is_deleting.set(false);
                    show_delete_modal.set(false);
                    nav_clone.push(Route::DashboardPage {});
                }
                Err(e) => {
                    is_deleting.set(false);
                    delete_error.set(Some(e.to_string()));
                }
            }
        });
    };

    rsx! {
        div { class: "dashboard-container",
            ProjectManageNav {
                project_id,
                owner_org_id: project.owner_org_id,
                is_leader,
            }

            div { style: "display: flex; flex-direction: column; gap: 24px;",
                ProjectManageHeader {
                    project: project.clone(),
                    on_invite: move |_| show_invite_modal.set(true),
                }

                ProjectAffiliatedOrgsSection {
                    project_id,
                    affiliated_orgs,
                    available_orgs,
                    form_error: (*form_error.read()).clone(),
                    is_affiliating: *is_affiliating.read(),
                    on_refresh: move |_| handle_refresh(),
                    on_remove: move |oid| handle_remove_org(oid),
                    on_affiliate: handle_affiliate_new,
                }

                ProjectMembersLedgerSection {
                    leader_id: project.leader_id,
                    project_members,
                    on_invite: move |_| show_invite_modal.set(true),
                }

                if is_leader {
                    ProjectDangerZone {
                        on_delete: move |_| show_delete_modal.set(true),
                    }
                }
            }
        }

        ProjectManageModalsHost {
            project_id,
            project_name: project.name.clone(),
            pending_warning: (*pending_warning.read()).clone(),
            is_warning_loading: *is_warning_loading.read(),
            show_invite_modal: *show_invite_modal.read(),
            show_delete_modal: *show_delete_modal.read(),
            is_deleting: *is_deleting.read(),
            delete_error: (*delete_error.read()).clone(),
            on_close_warning: move |_| pending_warning.set(None),
            on_confirm_warning: move |_| {
                if let Some((_, oid)) = (*pending_warning.read()).clone() {
                    is_warning_loading.set(true);
                    spawn(async move {
                        let _ = remove_organization_from_project(project_id, oid, true).await;
                        is_warning_loading.set(false);
                        pending_warning.set(None);
                        handle_refresh();
                    });
                }
            },
            on_close_invite: move |_| {
                show_invite_modal.set(false);
                handle_refresh();
            },
            on_close_delete: move |_| {
                show_delete_modal.set(false);
                delete_error.set(None);
            },
            on_confirm_delete: handle_confirm_delete,
        }
    }
}

/// Navigation bar and breadcrumb back to the board or organization dashboard.
#[component]
fn ProjectManageNav(project_id: i64, owner_org_id: Option<i64>, is_leader: bool) -> Element {
    rsx! {
        div { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 20px; flex-wrap: wrap; gap: 10px;",
            div { style: "display: flex; gap: 16px; align-items: center;",
                Link {
                    to: Route::ProjectTasksPage { project_id },
                    style: "color: var(--primary); text-decoration: none; font-size: var(--font-size-body-sm); font-weight: var(--font-weight-bold); display: inline-flex; align-items: center; gap: 6px;",
                    "← Back to Tasks Board"
                }
                if let Some(owner_oid) = owner_org_id {
                    Link {
                        to: Route::OrganizationDashboardPage { organization_id: owner_oid },
                        style: "color: var(--text-secondary); text-decoration: none; font-size: var(--font-size-body-sm);",
                        "🏢 Organization Dashboard"
                    }
                }
            }
            span { class: "task-detail-status status-accepted badge-sm",
                if is_leader { "Project Leader" } else { "Project Admin" }
            }
        }
    }
}

/// Header card displaying project metadata, leader, and quick action buttons.
#[component]
fn ProjectManageHeader(project: Project, on_invite: EventHandler<()>) -> Element {
    let pid = project.id;
    rsx! {
        div { class: "dashboard-panel",
            div { style: "display: flex; justify-content: space-between; align-items: flex-start; flex-wrap: wrap; gap: 16px;",
                div {
                    div { style: "display: flex; gap: 10px; align-items: center; margin-bottom: 4px;",
                        h1 { class: "dashboard-title", style: "margin: 0;", "{project.name}" }
                        span { class: "task-detail-status status-suggested badge-sm", "Project Administration" }
                    }
                    p { style: "margin: 0; color: var(--text-secondary); font-size: var(--font-size-body-sm);",
                        if project.description.is_empty() {
                            "No description provided."
                        } else {
                            "{project.description}"
                        }
                    }
                    div { style: "margin-top: 8px; font-size: var(--font-size-meta); color: var(--text-muted);",
                        "Project Leader: " strong { "{project.leader_name}" }
                        if let Some(org_name) = &project.owner_org_name {
                            " • Owner Organization: " strong { "{org_name}" }
                        }
                    }
                }
                div { style: "display: flex; gap: 10px;",
                    button {
                        r#type: "button",
                        class: "btn-task-edit",
                        onclick: move |_| on_invite.call(()),
                        "👥 + Invite Member"
                    }
                    Link {
                        to: Route::ProjectTasksPage { project_id: pid },
                        class: "btn-task-cancel",
                        style: "text-decoration: none;",
                        "View Kanban Board →"
                    }
                }
            }
        }
    }
}

/// Section listing affiliated organizations and providing form to affiliate additional organizations.
#[component]
fn ProjectAffiliatedOrgsSection(
    project_id: i64,
    affiliated_orgs: Vec<ProjectOrganization>,
    available_orgs: Vec<Organization>,
    form_error: Option<String>,
    is_affiliating: bool,
    on_refresh: EventHandler<()>,
    on_remove: EventHandler<i64>,
    on_affiliate: EventHandler<(i64, String, String)>,
) -> Element {
    rsx! {
        div { class: "dashboard-panel",
            div {
                h2 { class: "dashboard-panel-title", "Affiliated Organizations & Role Ceilings ({affiliated_orgs.len()})" }
                p { style: "margin: 4px 0 0; color: var(--text-muted); font-size: var(--font-size-body-sm);",
                    "Define what organizations are permitted to collaborate in this project. The Maximum Privilege Ceiling sets a strict upper bound on what privileges can be inherited by members of that organization."
                }
            }

            if let Some(err) = form_error {
                div { class: "task-detail-error", "{err}" }
            }

            if affiliated_orgs.is_empty() {
                p { class: "empty-description", "No external organizations are affiliated with this project yet." }
            } else {
                div { style: "display: flex; flex-direction: column; gap: 12px;",
                    for org in affiliated_orgs.iter() {
                        {
                            let o = org.clone();
                            let oid = o.organization_id;
                            rsx! {
                                ProjectManageOrgRow {
                                    project_id,
                                    org: o,
                                    on_update: move |_| on_refresh.call(()),
                                    on_remove: move |_| on_remove.call(oid),
                                }
                            }
                        }
                    }
                }
            }

            AffiliateNewOrgForm {
                available_orgs,
                is_affiliating,
                on_submit: move |args| on_affiliate.call(args),
            }
        }
    }
}

/// Form inputs to affiliate a new organization with a defined relationship and role ceiling.
#[component]
fn AffiliateNewOrgForm(
    available_orgs: Vec<Organization>,
    is_affiliating: bool,
    on_submit: EventHandler<(i64, String, String)>,
) -> Element {
    let mut new_org_id = use_signal(|| Option::<i64>::None);
    let mut new_rel_type = use_signal(|| "partner".to_string());
    let mut new_max_role = use_signal(|| "member".to_string());
    let mut validation_error = use_signal(|| Option::<String>::None);

    let handle_submit = move |_| {
        let oid = match *new_org_id.read() {
            Some(id) => id,
            None => {
                validation_error.set(Some(
                    "Please select an organization to affiliate.".to_string(),
                ));
                return;
            }
        };
        validation_error.set(None);
        let rel = new_rel_type.read().clone();
        let max_r = new_max_role.read().clone();
        on_submit.call((oid, rel, max_r));
    };

    rsx! {
        div { style: "border-top: 1px solid var(--border-subtle); padding-top: 16px; margin-top: 8px;",
            h3 { style: "margin: 0 0 12px 0; font-size: var(--font-size-cardheader-small); color: var(--text-primary);",
                "+ Affiliate New Organization"
            }
            if let Some(err) = &*validation_error.read() {
                div { class: "task-detail-error", "{err}" }
            }
            if available_orgs.is_empty() {
                p { style: "color: var(--text-muted); font-size: var(--font-size-meta); font-style: italic;",
                    "All registered organizations are already affiliated with this project."
                }
            } else {
                div { style: "display: grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)) auto; gap: 12px; align-items: flex-end;",
                    div {
                        label { class: "form-label", "Organization" }
                        select {
                            class: "task-status-select",
                            onchange: move |e| {
                                if let Ok(id) = e.value().parse::<i64>() {
                                    new_org_id.set(Some(id));
                                } else {
                                    new_org_id.set(None);
                                }
                            },
                            option { value: "", "-- Select organization --" }
                            for a in &available_orgs {
                                option { value: "{a.id}", "{a.name} (@{a.slug})" }
                            }
                        }
                    }
                    div {
                        label { class: "form-label", "Relationship Type" }
                        select {
                            class: "task-status-select",
                            value: "{new_rel_type}",
                            onchange: move |e| new_rel_type.set(e.value()),
                            option { value: "partner", "Partner" }
                            option { value: "owner", "Owner" }
                            option { value: "contractor", "Contractor" }
                            option { value: "auditor", "Auditor" }
                        }
                    }
                    div {
                        label { class: "form-label", "Max Privilege Ceiling" }
                        select {
                            class: "task-status-select",
                            value: "{new_max_role}",
                            onchange: move |e| new_max_role.set(e.value()),
                            option { value: "viewer", "Viewer (Read Only)" }
                            option { value: "member", "Member (Create & Edit Tasks)" }
                            option { value: "admin", "Admin (Manage Settings & Delete)" }
                        }
                    }
                    button {
                        r#type: "button",
                        class: "btn-task-save",
                        disabled: is_affiliating,
                        onclick: handle_submit,
                        if is_affiliating { "Affiliating..." } else { "+ Affiliate Org" }
                    }
                }
            }
        }
    }
}

/// Ledger section displaying all enrolled project members and their grant origins.
#[component]
fn ProjectMembersLedgerSection(
    leader_id: i64,
    project_members: Vec<ProjectMember>,
    on_invite: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "dashboard-panel",
            div { style: "display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 12px;",
                div {
                    h2 { class: "dashboard-panel-title", style: "margin: 0;", "Project Members & Grant Ledger ({project_members.len()})" }
                    p { style: "margin: 4px 0 0; color: var(--text-muted); font-size: var(--font-size-body-sm);",
                        "List of participants enrolled in this project, showing their effective privileges and enrollment source."
                    }
                }
                button {
                    r#type: "button",
                    class: "btn-action-sm",
                    onclick: move |_| on_invite.call(()),
                    "+ Invite Direct Member"
                }
            }

            div { style: "overflow-x: auto;",
                table { style: "width: 100%; border-collapse: collapse; text-align: left; font-size: var(--font-size-body-sm);",
                    thead {
                        tr { style: "border-bottom: 2px solid var(--border-default); color: var(--text-muted); font-size: var(--font-size-meta); text-transform: uppercase;",
                            th { style: "padding: 10px 12px;", "Member" }
                            th { style: "padding: 10px 12px;", "Project Role" }
                            th { style: "padding: 10px 12px;", "Enrollment Source" }
                            th { style: "padding: 10px 12px;", "Active Hat / Key" }
                        }
                    }
                    tbody {
                        for member in project_members.iter() {
                            {
                                let is_proj_leader = member.user_id == leader_id;
                                let m = member.clone();
                                rsx! {
                                    ProjectMemberLedgerRow {
                                        key: "{m.user_id}",
                                        member: m,
                                        is_proj_leader,
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

/// Single table row for a member in the project ledger.
#[component]
fn ProjectMemberLedgerRow(member: ProjectMember, is_proj_leader: bool) -> Element {
    rsx! {
        tr { style: "border-bottom: 1px solid var(--border-subtle);",
            td { style: "padding: 12px;",
                strong { style: "display: block; color: var(--text-primary);", "{member.user_name}" }
                span { style: "color: var(--text-muted); font-size: var(--font-size-meta);", "{member.user_email}" }
            }
            td { style: "padding: 12px;",
                if is_proj_leader {
                    span { class: "task-detail-status status-accepted badge-sm", "Project Leader" }
                } else {
                    span { class: "task-detail-status status-suggested badge-sm", "{member.role}" }
                }
            }
            td { style: "padding: 12px;",
                if member.granted_by_org_role {
                    span { style: "color: var(--primary); font-size: var(--font-size-meta); font-weight: var(--font-weight-bold);",
                        "🏢 Org Role Grant"
                    }
                } else {
                    span { style: "color: var(--text-muted); font-size: var(--font-size-meta);",
                        "👤 Direct Project Member"
                    }
                }
            }
            td { style: "padding: 12px;",
                span { style: "font-size: var(--font-size-meta); color: var(--text-muted); font-family: monospace;",
                    if let Some(key) = &member.active_grant_key {
                        "{key}"
                    } else {
                        "default"
                    }
                }
            }
        }
    }
}

/// Danger zone panel allowing the project leader to permanently delete the project.
#[component]
fn ProjectDangerZone(on_delete: EventHandler<()>) -> Element {
    rsx! {
        div { class: "dashboard-panel", style: "border: 1px solid var(--danger-bg);",
            h2 { class: "dashboard-panel-title", style: "color: var(--danger);", "Danger Zone" }
            div { style: "display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 12px; padding: 12px; background: var(--bg-surface-raised); border-radius: var(--radius-sm); border-left: 3px solid var(--danger);",
                div {
                    strong { style: "display: block; color: var(--danger);", "Delete Project" }
                    span { style: "font-size: var(--font-size-meta); color: var(--text-muted);",
                        "Permanently delete this project, all associated tasks, and membership grants. Requires typed confirmation."
                    }
                }
                button {
                    r#type: "button",
                    class: "btn-action-sm",
                    style: "color: var(--danger); border-color: var(--danger); font-weight: var(--font-weight-bold);",
                    onclick: move |_| on_delete.call(()),
                    "Delete Project"
                }
            }
        }
    }
}

/// Hosts modal dialogues for active task warnings, member invitations, and project deletion.
#[component]
fn ProjectManageModalsHost(
    project_id: i64,
    project_name: String,
    pending_warning: Option<(String, i64)>,
    is_warning_loading: bool,
    show_invite_modal: bool,
    show_delete_modal: bool,
    is_deleting: bool,
    delete_error: Option<String>,
    on_close_warning: EventHandler<()>,
    on_confirm_warning: EventHandler<()>,
    on_close_invite: EventHandler<()>,
    on_close_delete: EventHandler<()>,
    on_confirm_delete: EventHandler<String>,
) -> Element {
    rsx! {
        if let Some((msg, _)) = pending_warning {
            ConfirmActiveTasksWarningModal {
                warning_message: msg,
                on_close: move |_| on_close_warning.call(()),
                on_confirm: move |_| on_confirm_warning.call(()),
                is_loading: is_warning_loading,
            }
        }

        if show_invite_modal {
            InviteProjectMemberModal {
                project_id,
                project_name: Some(project_name.clone()),
                on_close: move |_| on_close_invite.call(()),
                on_invited: move |_| on_close_invite.call(()),
            }
        }

        if show_delete_modal {
            ConfirmDeleteModal {
                entity_type: "project".to_string(),
                entity_name: project_name,
                on_close: move |_| on_close_delete.call(()),
                on_confirm: move |confirmation| on_confirm_delete.call(confirmation),
                is_deleting,
                error: delete_error,
            }
        }
    }
}

/// Row representing an affiliated organization with editable relationship and role ceiling.
#[component]
fn ProjectManageOrgRow(
    project_id: i64,
    org: ProjectOrganization,
    on_update: EventHandler<()>,
    on_remove: EventHandler<()>,
) -> Element {
    let mut current_rel = use_signal(|| org.relationship_type.clone());
    let mut current_max = use_signal(|| org.max_granted_role.clone());
    let mut is_saving = use_signal(|| false);
    let is_dirty = use_memo(move || {
        current_rel() != org.relationship_type || current_max() != org.max_granted_role
    });

    let oid = org.organization_id;

    let handle_save = move |_| {
        let rel = current_rel.read().clone();
        let max_r = current_max.read().clone();
        is_saving.set(true);

        spawn(async move {
            if affiliate_organization_with_project(project_id, oid, rel, max_r)
                .await
                .is_ok()
            {
                is_saving.set(false);
                on_update.call(());
            } else {
                is_saving.set(false);
            }
        });
    };

    rsx! {
        div { class: "inbox-item",
            div { style: "display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 12px;",
                div { style: "display: flex; gap: 8px; align-items: center;",
                    span { class: "inbox-item-title", "{org.organization_name}" }
                }
                div { style: "display: flex; gap: 10px; align-items: center; flex-wrap: wrap;",
                    div { style: "display: flex; align-items: center; gap: 4px;",
                        span { style: "font-size: var(--font-size-meta); color: var(--text-muted);", "Relationship:" }
                        select {
                            class: "task-status-select",
                            style: "padding: 3px 6px; font-size: var(--font-size-meta);",
                            value: "{current_rel}",
                            onchange: move |e| current_rel.set(e.value()),
                            option { value: "owner", "Owner" }
                            option { value: "partner", "Partner" }
                            option { value: "contractor", "Contractor" }
                            option { value: "auditor", "Auditor" }
                        }
                    }
                    div { style: "display: flex; align-items: center; gap: 4px;",
                        span { style: "font-size: var(--font-size-meta); color: var(--text-muted);", "Max Role Ceiling:" }
                        select {
                            class: "task-status-select",
                            style: "padding: 3px 6px; font-size: var(--font-size-meta);",
                            value: "{current_max}",
                            onchange: move |e| current_max.set(e.value()),
                            option { value: "viewer", "Viewer" }
                            option { value: "member", "Member" }
                            option { value: "admin", "Admin" }
                        }
                    }
                    if is_dirty() {
                        button {
                            r#type: "button",
                            class: "btn-action-sm",
                            style: "background-color: var(--primary); color: #fff;",
                            disabled: *is_saving.read(),
                            onclick: handle_save,
                            if *is_saving.read() { "Saving..." } else { "Save Changes" }
                        }
                    }
                    button {
                        r#type: "button",
                        class: "btn-action-sm",
                        style: "color: var(--danger); border-color: var(--danger);",
                        onclick: move |_| on_remove.call(()),
                        "Revoke Access"
                    }
                }
            }
        }
    }
}
