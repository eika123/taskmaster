use crate::backend::app_endpoints::{
    affiliate_organization_with_project, get_available_organizations_for_project,
    get_project_organizations, remove_organization_from_project,
};
use crate::components::organizations::modals::ConfirmActiveTasksWarningModal;
use crate::types::ProjectOrganization;
use dioxus::prelude::*;

#[component]
pub fn ProjectOrganizationsModal(
    project_id: i64,
    project_name: String,
    on_close: EventHandler<()>,
) -> Element {
    let mut orgs_resource =
        use_server_future(move || async move { get_project_organizations(project_id).await })?;

    let mut available_resource = use_server_future(move || async move {
        get_available_organizations_for_project(project_id).await
    })?;

    let affiliated_orgs = match &*orgs_resource.read() {
        Some(Ok(orgs)) => orgs.clone(),
        _ => Vec::new(),
    };

    let available_orgs = match &*available_resource.read() {
        Some(Ok(orgs)) => orgs.clone(),
        _ => Vec::new(),
    };

    // State for affiliating a new organization
    let mut new_org_id = use_signal(|| Option::<i64>::None);
    let mut new_rel_type = use_signal(|| "partner".to_string());
    let mut new_max_role = use_signal(|| "member".to_string());
    let mut form_error = use_signal(|| Option::<String>::None);
    let mut is_affiliating = use_signal(|| false);

    // Warning confirmation modal state for active tasks: (warning_message, org_id)
    let mut pending_warning = use_signal(|| Option::<(String, i64)>::None);
    let mut is_warning_loading = use_signal(|| false);

    let handle_affiliate_new = move |_| {
        let oid = match *new_org_id.read() {
            Some(id) => id,
            None => {
                form_error.set(Some(
                    "Please select an organization to affiliate.".to_string(),
                ));
                return;
            }
        };

        let rel = new_rel_type.read().clone();
        let max_r = new_max_role.read().clone();

        is_affiliating.set(true);
        form_error.set(None);

        spawn(async move {
            match affiliate_organization_with_project(project_id, oid, rel, max_r).await {
                Ok(_) => {
                    is_affiliating.set(false);
                    new_org_id.set(None);
                    orgs_resource.restart();
                    available_resource.restart();
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
                Ok(_) => {
                    orgs_resource.restart();
                    available_resource.restart();
                }
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

    let handle_confirm_warning = move |_| {
        if let Some((_, oid)) = (*pending_warning.read()).clone() {
            is_warning_loading.set(true);
            spawn(async move {
                let _ = remove_organization_from_project(project_id, oid, true).await;
                is_warning_loading.set(false);
                pending_warning.set(None);
                orgs_resource.restart();
                available_resource.restart();
            });
        }
    };

    rsx! {
        div { class: "overlay-backdrop",
            onclick: move |_| on_close.call(()),
            div {
                class: "overlay-modal",
                style: "max-width: 680px;",
                onclick: move |e| e.stop_propagation(),
                div { class: "overlay-header",
                    div { class: "overlay-title-group",
                        h2 { class: "overlay-title", "Affiliated Organizations" }
                        p { class: "overlay-subtitle", "Manage organization partnerships, relationships, and max privilege ceilings for \"{project_name}\"." }
                    }
                    button {
                        r#type: "button",
                        class: "btn-close-overlay",
                        onclick: move |_| on_close.call(()),
                        "✕"
                    }
                }

                div { style: "padding: 20px 24px; display: flex; flex-direction: column; gap: 20px;",
                    if let Some(err) = &*form_error.read() {
                        div { class: "task-detail-error", "{err}" }
                    }

                    AffiliatedOrgsList {
                        project_id,
                        affiliated_orgs: affiliated_orgs.clone(),
                        on_update: move |_| {
                            orgs_resource.restart();
                        },
                        on_remove: handle_remove_org,
                    }

                    AffiliateOrgForm {
                        available_orgs: available_orgs.clone(),
                        is_affiliating: *is_affiliating.read(),
                        new_rel_type: new_rel_type.read().clone(),
                        new_max_role: new_max_role.read().clone(),
                        on_change_org: move |id| new_org_id.set(id),
                        on_change_rel: move |rel| new_rel_type.set(rel),
                        on_change_role: move |role| new_max_role.set(role),
                        on_submit: handle_affiliate_new,
                    }

                    div { class: "overlay-footer",
                        button {
                            r#type: "button",
                            class: "btn-task-cancel",
                            onclick: move |_| on_close.call(()),
                            "Done"
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
    }
}

/// Renders the list of current affiliated organizations.
#[component]
fn AffiliatedOrgsList(
    project_id: i64,
    affiliated_orgs: Vec<ProjectOrganization>,
    on_update: EventHandler<()>,
    on_remove: EventHandler<i64>,
) -> Element {
    rsx! {
        div {
            h3 { style: "margin: 0 0 10px 0; font-size: var(--font-size-cardheader-small); color: var(--text-primary);",
                "Current Affiliated Organizations ({affiliated_orgs.len()})"
            }
            if affiliated_orgs.is_empty() {
                p { style: "color: var(--text-muted); font-size: var(--font-size-body-sm);",
                    "No organizations are affiliated with this project yet."
                }
            } else {
                div { style: "display: flex; flex-direction: column; gap: 12px;",
                    for org in affiliated_orgs.into_iter() {
                        {
                            let oid = org.organization_id;
                            rsx! {
                                ProjectOrgRow {
                                    project_id,
                                    org,
                                    on_update: move |_| on_update.call(()),
                                    on_remove: move |_| on_remove.call(oid),
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Form for selecting and affiliating a new organization to a project.
#[component]
fn AffiliateOrgForm(
    available_orgs: Vec<crate::types::Organization>,
    is_affiliating: bool,
    new_rel_type: String,
    new_max_role: String,
    on_change_org: EventHandler<Option<i64>>,
    on_change_rel: EventHandler<String>,
    on_change_role: EventHandler<String>,
    on_submit: EventHandler<()>,
) -> Element {
    rsx! {
        div { style: "border-top: 1px solid var(--border-subtle); padding-top: 16px;",
            h3 { style: "margin: 0 0 12px 0; font-size: var(--font-size-cardheader-small); color: var(--text-primary);",
                "+ Affiliate New Organization"
            }
            if available_orgs.is_empty() {
                p { style: "color: var(--text-muted); font-size: var(--font-size-meta); font-style: italic;",
                    "All registered organizations are already affiliated with this project."
                }
            } else {
                div { style: "display: grid; grid-template-columns: 2fr 1fr 1fr auto; gap: 10px; align-items: flex-end;",
                    div {
                        label { class: "form-label", "Select Organization" }
                        select {
                            class: "task-status-select",
                            onchange: move |e| {
                                if let Ok(id) = e.value().parse::<i64>() {
                                    on_change_org.call(Some(id));
                                } else {
                                    on_change_org.call(None);
                                }
                            },
                            option { value: "", "-- Choose organization --" }
                            for a in &available_orgs {
                                option { value: "{a.id}", "{a.name} (@{a.slug})" }
                            }
                        }
                    }
                    div {
                        label { class: "form-label", "Relationship" }
                        select {
                            class: "task-status-select",
                            value: "{new_rel_type}",
                            onchange: move |e| on_change_rel.call(e.value()),
                            option { value: "partner", "Partner" }
                            option { value: "owner", "Owner" }
                            option { value: "contractor", "Contractor" }
                            option { value: "auditor", "Auditor" }
                        }
                    }
                    div {
                        label { class: "form-label", "Max Role" }
                        select {
                            class: "task-status-select",
                            value: "{new_max_role}",
                            onchange: move |e| on_change_role.call(e.value()),
                            option { value: "viewer", "Viewer" }
                            option { value: "member", "Member" }
                            option { value: "admin", "Admin" }
                        }
                    }
                    button {
                        r#type: "button",
                        class: "btn-task-save",
                        disabled: is_affiliating,
                        onclick: move |_| on_submit.call(()),
                        if is_affiliating { "Adding..." } else { "+ Affiliate" }
                    }
                }
            }
        }
    }
}

#[component]
fn ProjectOrgRow(
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
            div { style: "display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 10px;",
                div { style: "display: flex; gap: 8px; align-items: center;",
                    span { class: "inbox-item-title", "{org.organization_name}" }
                }
                div { style: "display: flex; gap: 8px; align-items: center;",
                    div { style: "display: flex; align-items: center; gap: 4px;",
                        span { style: "font-size: var(--font-size-meta); color: var(--text-muted);", "Rel:" }
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
                        span { style: "font-size: var(--font-size-meta); color: var(--text-muted);", "Max Role:" }
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
                            if *is_saving.read() { "Saving..." } else { "Save" }
                        }
                    }
                    button {
                        r#type: "button",
                        class: "btn-action-sm",
                        style: "color: var(--danger); border-color: var(--danger);",
                        onclick: move |_| on_remove.call(()),
                        "Remove"
                    }
                }
            }
        }
    }
}
