use crate::backend::app_endpoints::{create_organization, create_project, delete_organization};
use crate::types::Organization;
use dioxus::prelude::*;

#[component]
pub fn CreateProjectModal(
    organizations: Vec<Organization>,
    on_close: EventHandler<()>,
    on_created: EventHandler<()>,
) -> Element {
    let mut new_proj_name = use_signal(String::new);
    let mut new_proj_desc = use_signal(String::new);
    let mut new_proj_org_id = use_signal(|| Option::<i64>::None);
    let mut new_proj_template = use_signal(|| "classic_kanban".to_string());
    let mut proj_form_error = use_signal(|| Option::<String>::None);
    let mut is_creating_proj = use_signal(|| false);

    let handle_create = move |_| {
        let name = new_proj_name.read().trim().to_string();
        let desc = new_proj_desc.read().trim().to_string();
        let org_id = *new_proj_org_id.read();
        let template = new_proj_template.read().clone();

        if name.is_empty() {
            proj_form_error.set(Some("Project name is required.".to_string()));
            return;
        }

        is_creating_proj.set(true);
        proj_form_error.set(None);

        spawn(async move {
            match create_project(name, desc, org_id, Some(template)).await {
                Ok(_) => {
                    new_proj_name.write().clear();
                    new_proj_desc.write().clear();
                    new_proj_org_id.set(None);
                    new_proj_template.set("classic_kanban".to_string());
                    is_creating_proj.set(false);
                    on_created.call(());
                }
                Err(err) => {
                    proj_form_error.set(Some(err.to_string()));
                    is_creating_proj.set(false);
                }
            }
        });
    };

    rsx! {
        div { class: "dashboard-panel", style: "border: 1px solid var(--primary);",
            h2 { class: "dashboard-panel-title", "Create a New Project" }
            if let Some(err) = &*proj_form_error.read() {
                div { class: "task-detail-error", "{err}" }
            }
            CreateProjectForm {
                name: new_proj_name.read().clone(),
                desc: new_proj_desc.read().clone(),
                template: new_proj_template.read().clone(),
                organizations,
                is_creating: *is_creating_proj.read(),
                on_name_change: move |val| new_proj_name.set(val),
                on_desc_change: move |val| new_proj_desc.set(val),
                on_template_change: move |val| new_proj_template.set(val),
                on_org_change: move |val| new_proj_org_id.set(val),
                on_cancel: move |_| on_close.call(()),
                on_submit: handle_create,
            }
        }
    }
}

/// Form controls for project creation.
#[component]
fn CreateProjectForm(
    name: String,
    desc: String,
    template: String,
    organizations: Vec<Organization>,
    is_creating: bool,
    on_name_change: EventHandler<String>,
    on_desc_change: EventHandler<String>,
    on_template_change: EventHandler<String>,
    on_org_change: EventHandler<Option<i64>>,
    on_cancel: EventHandler<()>,
    on_submit: EventHandler<()>,
) -> Element {
    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 12px;",
            div {
                label { class: "form-label", "Project Name" }
                input {
                    class: "task-edit-title-input",
                    placeholder: "e.g. Core Engine",
                    value: "{name}",
                    oninput: move |e| on_name_change.call(e.value()),
                }
            }
            div {
                label { class: "form-label", "Description" }
                textarea {
                    class: "task-edit-description-textarea",
                    placeholder: "What will this project accomplish?",
                    value: "{desc}",
                    oninput: move |e| on_desc_change.call(e.value()),
                }
            }
            div {
                label { class: "form-label", "Workflow Pipeline Template" }
                select {
                    class: "task-status-select",
                    value: "{template}",
                    onchange: move |e| on_template_change.call(e.value()),
                    option { value: "classic_kanban", "Classic Kanban (Backlog, In Progress [WIP 5], Review [WIP 3], Done)" }
                    option { value: "software_sprint", "Software Sprint (Triage, Backlog, Dev [WIP 4], QA [WIP 3], Deployed)" }
                    option { value: "guild_raid_prep", "Guild Raid Prep (Signups, Roster, Consumables, Ready, Raid Complete)" }
                    option { value: "legacy_flow", "Legacy Flow (Suggested, Accepted, Assigned, InProgress, Finished)" }
                }
            }
            if !organizations.is_empty() {
                div {
                    label { class: "form-label", "Owning Organization (Optional)" }
                    select {
                        class: "task-status-select",
                        onchange: move |e| {
                            let val = e.value();
                            if val.is_empty() {
                                on_org_change.call(None);
                            } else if let Ok(org_id) = val.parse::<i64>() {
                                on_org_change.call(Some(org_id));
                            }
                        },
                        option { value: "", "Standalone (Personal Project)" }
                        for org in &organizations {
                            option { value: "{org.id}", "{org.name}" }
                        }
                    }
                }
            }
            div { style: "display: flex; gap: 8px; justify-content: flex-end;",
                button {
                    r#type: "button",
                    class: "btn-task-cancel",
                    onclick: move |_| on_cancel.call(()),
                    "Cancel"
                }
                button {
                    r#type: "button",
                    class: "btn-task-save",
                    disabled: is_creating,
                    onclick: move |_| on_submit.call(()),
                    if is_creating { "Creating..." } else { "Create Project" }
                }
            }
        }
    }
}

#[component]
pub fn CreateOrgModal(on_close: EventHandler<()>, on_created: EventHandler<()>) -> Element {
    let mut new_org_name = use_signal(String::new);
    let mut new_org_slug = use_signal(String::new);
    let mut org_form_error = use_signal(|| Option::<String>::None);
    let mut is_creating_org = use_signal(|| false);

    let handle_create = move |_| {
        let name = new_org_name.read().trim().to_string();
        let slug = new_org_slug.read().trim().to_lowercase();

        if name.is_empty() || slug.is_empty() {
            org_form_error.set(Some("Name and slug are required.".to_string()));
            return;
        }

        is_creating_org.set(true);
        org_form_error.set(None);

        spawn(async move {
            match create_organization(name, slug).await {
                Ok(_) => {
                    new_org_name.write().clear();
                    new_org_slug.write().clear();
                    is_creating_org.set(false);
                    on_created.call(());
                }
                Err(err) => {
                    org_form_error.set(Some(err.to_string()));
                    is_creating_org.set(false);
                }
            }
        });
    };

    rsx! {
        div { class: "dashboard-panel", style: "border: 1px solid var(--primary);",
            h2 { class: "dashboard-panel-title", "Create a New Organization" }
            if let Some(err) = &*org_form_error.read() {
                div { class: "task-detail-error", "{err}" }
            }
            div { style: "display: flex; flex-direction: column; gap: 12px;",
                div {
                    label { class: "form-label", "Organization Name" }
                    input {
                        class: "task-edit-title-input",
                        placeholder: "e.g. Acme Corporation",
                        value: "{new_org_name}",
                        oninput: move |e| new_org_name.set(e.value()),
                    }
                }
                div {
                    label { class: "form-label", "Unique Slug" }
                    input {
                        class: "task-edit-title-input",
                        placeholder: "e.g. acme-corp",
                        value: "{new_org_slug}",
                        oninput: move |e| new_org_slug.set(e.value()),
                    }
                }
                div { style: "display: flex; gap: 8px; justify-content: flex-end;",
                    button {
                        r#type: "button",
                        class: "btn-task-cancel",
                        onclick: move |_| on_close.call(()),
                        "Cancel"
                    }
                    button {
                        r#type: "button",
                        class: "btn-task-save",
                        disabled: *is_creating_org.read(),
                        onclick: handle_create,
                        if *is_creating_org.read() { "Creating..." } else { "Create Organization" }
                    }
                }
            }
        }
    }
}

#[component]
pub fn ManageOrgModal(
    organization: Organization,
    on_close: EventHandler<()>,
    on_deleted: EventHandler<()>,
) -> Element {
    let mut show_confirm_delete = use_signal(|| false);
    let mut delete_error = use_signal(|| Option::<String>::None);
    let mut is_deleting = use_signal(|| false);

    let org_id = organization.id;
    let org_name = organization.name.clone();

    rsx! {
        if *show_confirm_delete.read() {
            ConfirmDeleteModal {
                entity_type: "organization".to_string(),
                entity_name: org_name.clone(),
                on_close: move |_| show_confirm_delete.set(false),
                on_confirm: move |confirmation: String| {
                    is_deleting.set(true);
                    delete_error.set(None);
                    spawn(async move {
                        match delete_organization(org_id, confirmation).await {
                            Ok(_) => {
                                is_deleting.set(false);
                                show_confirm_delete.set(false);
                                on_deleted.call(());
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

        div { class: "overlay-backdrop",
            onclick: move |_| on_close.call(()),
            div {
                class: "overlay-modal",
                style: "max-width: 540px;",
                onclick: move |e| e.stop_propagation(),
                div { class: "overlay-header",
                    div { class: "overlay-title-group",
                        h2 { class: "overlay-title", "Manage {organization.name}" }
                        p { class: "overlay-subtitle", "@{organization.slug}" }
                    }
                    button {
                        r#type: "button",
                        class: "btn-close-overlay",
                        onclick: move |_| on_close.call(()),
                        "✕"
                    }
                }
                div { style: "padding: 20px 24px; display: flex; flex-direction: column; gap: 20px;",
                    div { class: "form-group",
                        label { class: "form-label", "Organization Details" }
                        div { style: "background: var(--bg-surface-raised); padding: 12px 14px; border-radius: var(--radius-sm); font-size: var(--font-size-body-sm);",
                            p { style: "margin: 0 0 6px 0;", "Governor: " strong { "{organization.governing_user_name}" } }
                            p { style: "margin: 0;", "Slug: " strong { "@{organization.slug}" } }
                        }
                    }

                    div { style: "border: 1px solid var(--danger-bg); border-radius: var(--radius-md); padding: 16px; background-color: var(--bg-surface);",
                        h3 { style: "margin: 0 0 8px 0; font-size: var(--font-size-cardheader-small); color: var(--danger);",
                            "Danger Zone"
                        }
                        p { style: "margin: 0 0 12px 0; font-size: var(--font-size-caption); color: var(--text-secondary);",
                            "Deleting an organization removes all roles, memberships, and records. It requires that all other members have left or been removed first."
                        }
                        button {
                            r#type: "button",
                            class: "btn-action-sm",
                            style: "color: var(--danger); border-color: var(--danger); font-weight: var(--font-weight-bold);",
                            onclick: move |_| show_confirm_delete.set(true),
                            "Delete this organization"
                        }
                    }

                    div { class: "overlay-footer",
                        button {
                            r#type: "button",
                            class: "btn-cancel",
                            onclick: move |_| on_close.call(()),
                            "Close"
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn ConfirmDeleteModal(
    entity_type: String,
    entity_name: String,
    on_close: EventHandler<()>,
    on_confirm: EventHandler<String>,
    is_deleting: bool,
    error: Option<String>,
) -> Element {
    let mut input_text = use_signal(String::new);
    let expected = format!("DELETE {}", entity_name.trim());
    let expected_check = expected.clone();
    let is_matched = use_memo(move || input_text().trim() == expected_check);

    rsx! {
        div { class: "overlay-backdrop", style: "z-index: 1100;",
            onclick: move |_| on_close.call(()),
            div {
                class: "overlay-modal",
                style: "max-width: 500px; border-top: 4px solid var(--danger);",
                onclick: move |e| e.stop_propagation(),
                div { class: "overlay-header",
                    div { class: "overlay-title-group",
                        h2 { class: "overlay-title", style: "color: var(--danger);", "Delete {entity_type}?" }
                        p { class: "overlay-subtitle", "This action cannot be undone." }
                    }
                    button {
                        r#type: "button",
                        class: "btn-close-overlay",
                        onclick: move |_| on_close.call(()),
                        disabled: is_deleting,
                        "✕"
                    }
                }
                div { style: "padding: 20px 24px; display: flex; flex-direction: column; gap: 14px;",
                    if let Some(err) = &error {
                        div { class: "task-detail-error", style: "padding: 10px;",
                            "{err}"
                        }
                    }
                    div { style: "background-color: var(--danger-bg); border-left: 3px solid var(--danger); padding: 12px 14px; border-radius: var(--radius-sm); font-size: var(--font-size-body-sm);",
                        p { style: "margin: 0; color: var(--text-primary);",
                            "Unexpected bad things will happen if you don’t read this! This will permanently delete the "
                            strong { "{entity_name}" }
                            " {entity_type} and all associated data."
                        }
                    }
                    p { style: "font-size: var(--font-size-body-sm); margin: 0;",
                        "Please type "
                        strong { style: "font-family: monospace; background: var(--bg-surface-raised); padding: 2px 6px; border-radius: 4px; color: var(--danger);",
                            "{expected}"
                        }
                        " to confirm:"
                    }
                    input {
                        class: "form-input",
                        style: "width: 100%; font-family: monospace;",
                        placeholder: "{expected}",
                        value: "{input_text}",
                        oninput: move |e| input_text.set(e.value()),
                        disabled: is_deleting,
                    }
                    div { class: "overlay-footer",
                        button {
                            r#type: "button",
                            class: "btn-cancel",
                            onclick: move |_| on_close.call(()),
                            disabled: is_deleting,
                            "Cancel"
                        }
                        button {
                            r#type: "button",
                            class: "btn-action-sm",
                            style: "background-color: var(--danger); color: white; border: none; padding: 9px 18px; font-weight: var(--font-weight-bold);",
                            disabled: is_deleting || !is_matched(),
                            onclick: move |_| on_confirm.call(input_text().trim().to_string()),
                            if is_deleting { "Deleting..." } else { "Delete this {entity_type}" }
                        }
                    }
                }
            }
        }
    }
}
