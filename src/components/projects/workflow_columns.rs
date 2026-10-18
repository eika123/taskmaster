use crate::backend::app_endpoints::{
    apply_workflow_template, create_workflow_status, delete_workflow_status,
    get_project_workflow_statuses, get_workflow_templates, reorder_workflow_statuses,
};
use crate::types::{BadgeVariant, StatusCategory, WorkflowStatusDto, WorkflowTemplate};
use dioxus::prelude::*;

/// Workflow columns management section for a project.
#[component]
pub fn ProjectWorkflowColumnsSection(project_id: i64, is_leader: bool) -> Element {
    let statuses_resource =
        use_server_future(move || async move { get_project_workflow_statuses(project_id).await })?;

    let templates_resource = use_server_future(move || async move { get_workflow_templates().await })?;

    let mut action_error = use_signal(|| Option::<String>::None);
    let mut is_mutating = use_signal(|| false);

    // Modal state for deleting a status with tasks
    let mut pending_delete = use_signal(|| Option::<WorkflowStatusDto>::None);

    let statuses = match &*statuses_resource.read() {
        Some(Ok(s)) => s.clone(),
        _ => Vec::new(),
    };

    let templates = match &*templates_resource.read() {
        Some(Ok(t)) => t.clone(),
        _ => Vec::new(),
    };

    let handle_move_up = move |idx: usize| {
        let mut is_mutating = is_mutating;
        let mut action_error = action_error;
        let current_statuses = match &*statuses_resource.read() {
            Some(Ok(s)) => s.clone(),
            _ => Vec::new(),
        };
        if idx == 0 || idx >= current_statuses.len() {
            return;
        }
        let mut ids: Vec<i64> = current_statuses.iter().map(|s| s.id).collect();
        ids.swap(idx, idx - 1);
        is_mutating.set(true);
        action_error.set(None);

        spawn(async move {
            match reorder_workflow_statuses(project_id, ids).await {
                Ok(_) => {
                    is_mutating.set(false);
                    let mut r = statuses_resource;
                    r.restart();
                }
                Err(e) => {
                    is_mutating.set(false);
                    action_error.set(Some(e.to_string()));
                }
            }
        });
    };

    let handle_move_down = move |idx: usize| {
        let mut is_mutating = is_mutating;
        let mut action_error = action_error;
        let current_statuses = match &*statuses_resource.read() {
            Some(Ok(s)) => s.clone(),
            _ => Vec::new(),
        };
        if idx + 1 >= current_statuses.len() {
            return;
        }
        let mut ids: Vec<i64> = current_statuses.iter().map(|s| s.id).collect();
        ids.swap(idx, idx + 1);
        is_mutating.set(true);
        action_error.set(None);

        spawn(async move {
            match reorder_workflow_statuses(project_id, ids).await {
                Ok(_) => {
                    is_mutating.set(false);
                    let mut r = statuses_resource;
                    r.restart();
                }
                Err(e) => {
                    is_mutating.set(false);
                    action_error.set(Some(e.to_string()));
                }
            }
        });
    };

    let handle_delete_click = move |status: WorkflowStatusDto| {
        let mut is_mutating = is_mutating;
        let mut action_error = action_error;
        let mut pending_delete = pending_delete;
        if status.task_count > 0 {
            pending_delete.set(Some(status));
        } else {
            is_mutating.set(true);
            action_error.set(None);
            let sid = status.id;
            spawn(async move {
                match delete_workflow_status(sid, None).await {
                    Ok(_) => {
                        is_mutating.set(false);
                        let mut r = statuses_resource;
                        r.restart();
                    }
                    Err(e) => {
                        is_mutating.set(false);
                        action_error.set(Some(e.to_string()));
                    }
                }
            });
        }
    };

    let handle_remap_and_delete = move |(del_id, remap_id): (i64, i64)| {
        is_mutating.set(true);
        action_error.set(None);
        pending_delete.set(None);

        spawn(async move {
            match delete_workflow_status(del_id, Some(remap_id)).await {
                Ok(_) => {
                    is_mutating.set(false);
                    let mut r = statuses_resource;
                    r.restart();
                }
                Err(e) => {
                    is_mutating.set(false);
                    action_error.set(Some(e.to_string()));
                }
            }
        });
    };

    rsx! {
        div { class: "dashboard-panel",
            div { style: "display: flex; justify-content: space-between; align-items: flex-start; flex-wrap: wrap; gap: 12px; margin-bottom: 16px;",
                div {
                    h2 { class: "dashboard-panel-title", "Workflow Columns & Process Pipeline ({statuses.len()})" }
                    p { style: "margin: 4px 0 0; color: var(--text-muted); font-size: var(--font-size-body-sm);",
                        "Customize the progression stages of tasks, define work-in-progress (WIP) limits, and set semantic guardrails."
                    }
                }
                if is_leader && !templates.is_empty() {
                    WorkflowTemplateSelector {
                        project_id,
                        templates: templates.clone(),
                        on_applied: move |_| {
                            let mut r = statuses_resource;
                            r.restart();
                        },
                    }
                }
            }

            if let Some(err) = action_error.read().as_ref() {
                div { class: "task-detail-error", style: "margin-bottom: 16px;", "{err}" }
            }

            // Columns table
            div { style: "overflow-x: auto; margin-bottom: 24px;",
                table { class: "org-members-table",
                    thead {
                        tr {
                            th { style: "width: 48px;", "#" }
                            th { "Column Name" }
                            th { "Category" }
                            th { "Badge Preview" }
                            th { "WIP Limit" }
                            th { "Tasks" }
                            th { "Semantic Invariants" }
                            if is_leader {
                                th { style: "text-align: right; width: 140px;", "Actions" }
                            }
                        }
                    }
                    tbody {
                        for (idx, status) in statuses.iter().enumerate() {
                            WorkflowStatusTableRow {
                                key: "{status.id}",
                                idx,
                                total_count: statuses.len(),
                                status: status.clone(),
                                is_leader,
                                on_move_up: move |i| handle_move_up(i),
                                on_move_down: move |i| handle_move_down(i),
                                on_delete: move |s| handle_delete_click(s),
                            }
                        }
                    }
                }
            }

            if is_leader {
                AddWorkflowStatusForm {
                    project_id,
                    on_created: move |_| {
                        let mut r = statuses_resource;
                        r.restart();
                    },
                }
            }
        }

        if let Some(del_status) = pending_delete.read().as_ref() {
            DeleteStatusRemapModal {
                status: del_status.clone(),
                available_targets: statuses.iter().filter(|s| s.id != del_status.id).cloned().collect(),
                on_cancel: move |_| pending_delete.set(None),
                on_confirm: handle_remap_and_delete,
            }
        }
    }
}

/// Single row representing a workflow status column.
#[component]
fn WorkflowStatusTableRow(
    idx: usize,
    total_count: usize,
    status: WorkflowStatusDto,
    is_leader: bool,
    on_move_up: EventHandler<usize>,
    on_move_down: EventHandler<usize>,
    on_delete: EventHandler<WorkflowStatusDto>,
) -> Element {
    let badge_cls = format!("badge-variant-{}", status.badge_variant.as_str());

    rsx! {
        tr {
            td { style: "color: var(--text-muted); font-size: var(--font-size-meta);", "{idx + 1}" }
            td {
                strong { "{status.name}" }
                if status.is_initial {
                    span {
                        class: "task-column-badge",
                        style: "margin-left: 8px; font-size: 10px; background: rgba(59, 130, 246, 0.2); color: #60a5fa;",
                        "Initial Landing"
                    }
                }
            }
            td {
                span { style: "text-transform: capitalize; font-size: var(--font-size-body-sm); color: var(--text-secondary);",
                    "{status.category.as_str()}"
                }
            }
            td {
                span { class: "task-column-badge {badge_cls}", "{status.name}" }
            }
            td {
                if let Some(wip) = status.wip_limit {
                    span {
                        class: if status.wip_exceeded { "wip-badge" } else { "" },
                        style: if !status.wip_exceeded { "font-size: var(--font-size-meta); color: var(--text-secondary);" } else { "" },
                        "Max {wip}"
                    }
                } else {
                    span { style: "color: var(--text-muted); font-size: var(--font-size-meta);", "None" }
                }
            }
            td {
                span {
                    style: if status.wip_exceeded { "color: #f59e0b; font-weight: bold;" } else { "color: var(--text-primary);" },
                    "{status.task_count}"
                }
            }
            td {
                div { style: "display: flex; gap: 4px; flex-wrap: wrap;",
                    if status.allows_assignment {
                        span { class: "badge-sm", style: "background: rgba(16, 185, 129, 0.15); color: #34d399; font-size: 10px;", "Assignable" }
                    } else {
                        span { class: "badge-sm", style: "background: rgba(107, 114, 128, 0.2); color: var(--text-muted); font-size: 10px;", "No Assign" }
                    }
                    if status.requires_assignee {
                        span { class: "badge-sm", style: "background: rgba(239, 68, 68, 0.15); color: #f87171; font-size: 10px;", "Needs Assignee" }
                    }
                }
            }
            if is_leader {
                td { style: "text-align: right;",
                    div { style: "display: inline-flex; gap: 4px;",
                        button {
                            r#type: "button",
                            class: "btn-task-cancel",
                            style: "padding: 2px 6px; font-size: 11px;",
                            disabled: idx == 0,
                            onclick: move |_| on_move_up.call(idx),
                            "↑"
                        }
                        button {
                            r#type: "button",
                            class: "btn-task-cancel",
                            style: "padding: 2px 6px; font-size: 11px;",
                            disabled: idx + 1 >= total_count,
                            onclick: move |_| on_move_down.call(idx),
                            "↓"
                        }
                        button {
                            r#type: "button",
                            class: "btn-task-cancel",
                            style: "padding: 2px 8px; font-size: 11px; color: var(--danger); border-color: rgba(239, 68, 68, 0.3);",
                            disabled: status.is_initial || total_count <= 1,
                            title: if status.is_initial { "Cannot delete initial landing column" } else { "Delete Column" },
                            onclick: {
                                let st = status.clone();
                                move |_| on_delete.call(st.clone())
                            },
                            "✕"
                        }
                    }
                }
            }
        }
    }
}

/// Form to create a new workflow status column.
#[component]
fn AddWorkflowStatusForm(project_id: i64, on_created: EventHandler<()>) -> Element {
    let mut name = use_signal(|| String::new());
    let mut category = use_signal(|| StatusCategory::Todo);
    let mut badge_variant = use_signal(|| BadgeVariant::Default);
    let mut wip_limit_str = use_signal(|| String::new());
    let mut allows_assignment = use_signal(|| true);
    let mut requires_assignee = use_signal(|| false);
    let mut is_submitting = use_signal(|| false);
    let mut form_error = use_signal(|| Option::<String>::None);

    let handle_submit = move |_e: FormEvent| {
        let name_val = name.read().trim().to_string();
        if name_val.is_empty() {
            form_error.set(Some("Column name is required".to_string()));
            return;
        }

        let wip_limit = if wip_limit_str.read().trim().is_empty() {
            None
        } else {
            match wip_limit_str.read().trim().parse::<i32>() {
                Ok(w) if w > 0 => Some(w),
                _ => {
                    form_error.set(Some("WIP limit must be a positive number".to_string()));
                    return;
                }
            }
        };

        is_submitting.set(true);
        form_error.set(None);
        let cat = *category.read();
        let bv = *badge_variant.read();
        let allows = *allows_assignment.read();
        let req = *requires_assignee.read();

        spawn(async move {
            match create_workflow_status(project_id, name_val, cat, bv, wip_limit, allows, req).await {
                Ok(_) => {
                    is_submitting.set(false);
                    name.set(String::new());
                    wip_limit_str.set(String::new());
                    on_created.call(());
                }
                Err(err) => {
                    is_submitting.set(false);
                    form_error.set(Some(err.to_string()));
                }
            }
        });
    };

    rsx! {
        div { style: "background: var(--bg-surface-raised); border: 1px dashed var(--border-color); border-radius: var(--radius-md); padding: 16px;",
            h3 { style: "margin: 0 0 12px; font-size: var(--font-size-heading-sm);", "+ Add Workflow Column" }

            if let Some(err) = form_error.read().as_ref() {
                div { class: "task-detail-error", style: "margin-bottom: 12px;", "{err}" }
            }

            form {
                onsubmit: handle_submit,
                style: "display: flex; gap: 12px; flex-wrap: wrap; align-items: flex-end;",

                div { style: "flex: 2; min-width: 140px;",
                    label { class: "dashboard-form-label", "Name" }
                    input {
                        class: "dashboard-form-input",
                        r#type: "text",
                        placeholder: "e.g., Code Review",
                        value: "{name}",
                        oninput: move |e| name.set(e.value()),
                    }
                }

                div { style: "flex: 1; min-width: 110px;",
                    label { class: "dashboard-form-label", "Category" }
                    select {
                        class: "dashboard-form-input",
                        value: "{category.read().as_str()}",
                        onchange: move |e| {
                            let cat = match e.value().as_str() {
                                "in_progress" => StatusCategory::InProgress,
                                "review" => StatusCategory::Review,
                                "done" => StatusCategory::Done,
                                _ => StatusCategory::Todo,
                            };
                            category.set(cat);
                        },
                        option { value: "todo", "To Do" }
                        option { value: "in_progress", "In Progress" }
                        option { value: "review", "Review" }
                        option { value: "done", "Done" }
                    }
                }

                div { style: "flex: 1; min-width: 110px;",
                    label { class: "dashboard-form-label", "Badge Color" }
                    select {
                        class: "dashboard-form-input",
                        value: "{badge_variant.read().as_str()}",
                        onchange: move |e| {
                            let bv = match e.value().as_str() {
                                "purple" => BadgeVariant::Purple,
                                "success" => BadgeVariant::Success,
                                "warning" => BadgeVariant::Warning,
                                "danger" => BadgeVariant::Danger,
                                "info" => BadgeVariant::Info,
                                "teal" => BadgeVariant::Teal,
                                _ => BadgeVariant::Default,
                            };
                            badge_variant.set(bv);
                        },
                        option { value: "default", "Default (Gray)" }
                        option { value: "info", "Blue (Info)" }
                        option { value: "purple", "Purple" }
                        option { value: "warning", "Amber (Warning)" }
                        option { value: "success", "Green (Success)" }
                        option { value: "danger", "Red (Danger)" }
                        option { value: "teal", "Teal" }
                    }
                }

                div { style: "width: 80px;",
                    label { class: "dashboard-form-label", "WIP Limit" }
                    input {
                        class: "dashboard-form-input",
                        r#type: "number",
                        min: "1",
                        placeholder: "∞",
                        value: "{wip_limit_str}",
                        oninput: move |e| wip_limit_str.set(e.value()),
                    }
                }

                div { style: "display: flex; gap: 12px; align-items: center; padding-bottom: 8px;",
                    label { style: "display: flex; align-items: center; gap: 4px; font-size: var(--font-size-body-sm); cursor: pointer;",
                        input {
                            r#type: "checkbox",
                            checked: *allows_assignment.read(),
                            onchange: move |e| allows_assignment.set(e.value() == "true"),
                        }
                        "Assignable"
                    }
                    label { style: "display: flex; align-items: center; gap: 4px; font-size: var(--font-size-body-sm); cursor: pointer;",
                        input {
                            r#type: "checkbox",
                            checked: *requires_assignee.read(),
                            onchange: move |e| requires_assignee.set(e.value() == "true"),
                        }
                        "Requires Assignee"
                    }
                }

                button {
                    r#type: "submit",
                    class: "btn-task-save",
                    disabled: *is_submitting.read(),
                    if *is_submitting.read() { "Adding..." } else { "Add Column" }
                }
            }
        }
    }
}

/// Selector dropdown to apply a starter workflow template to the project.
#[component]
fn WorkflowTemplateSelector(
    project_id: i64,
    templates: Vec<WorkflowTemplate>,
    on_applied: EventHandler<()>,
) -> Element {
    let mut selected_template = use_signal(|| templates.first().map(|t| t.id.clone()).unwrap_or_default());
    let mut is_applying = use_signal(|| false);

    let handle_apply = move |_| {
        let tid = selected_template.read().clone();
        if tid.is_empty() {
            return;
        }
        is_applying.set(true);

        spawn(async move {
            if let Ok(_) = apply_workflow_template(project_id, tid).await {
                is_applying.set(false);
                on_applied.call(());
            } else {
                is_applying.set(false);
            }
        });
    };

    rsx! {
        div { style: "display: inline-flex; gap: 8px; align-items: center; background: var(--bg-surface-raised); padding: 4px 8px; border-radius: var(--radius-md); border: 1px solid var(--border-color);",
            span { style: "font-size: var(--font-size-meta); color: var(--text-secondary); white-space: nowrap;", "Template:" }
            select {
                class: "dashboard-form-input",
                style: "padding: 4px 8px; font-size: var(--font-size-body-sm); width: auto;",
                value: "{selected_template}",
                onchange: move |e| selected_template.set(e.value()),
                for t in &templates {
                    option { key: "{t.id}", value: "{t.id}", "{t.name}" }
                }
            }
            button {
                r#type: "button",
                class: "btn-task-cancel",
                style: "padding: 4px 10px; font-size: var(--font-size-meta); white-space: nowrap;",
                disabled: *is_applying.read(),
                onclick: handle_apply,
                if *is_applying.read() { "Applying..." } else { "Apply Template" }
            }
        }
    }
}

/// Modal prompting the user to select a fallback status when deleting a column that contains tasks.
#[component]
fn DeleteStatusRemapModal(
    status: WorkflowStatusDto,
    available_targets: Vec<WorkflowStatusDto>,
    on_cancel: EventHandler<()>,
    on_confirm: EventHandler<(i64, i64)>,
) -> Element {
    let mut target_id = use_signal(|| available_targets.first().map(|s| s.id).unwrap_or(0));

    rsx! {
        div { class: "modal-backdrop",
            div { class: "modal-content", style: "max-width: 480px;",
                div { class: "modal-header",
                    h3 { class: "modal-title", "Delete Column: {status.name}" }
                    button {
                        class: "modal-close-btn",
                        onclick: move |_| on_cancel.call(()),
                        "×"
                    }
                }
                div { class: "modal-body",
                    p { style: "color: var(--text-secondary); margin-bottom: 16px;",
                        "Column " strong { "{status.name}" } " currently contains "
                        strong { "{status.task_count} tasks" } ". Select where to remap these tasks before deleting this column:"
                    }

                    div { style: "margin-bottom: 20px;",
                        label { class: "dashboard-form-label", "Remap Tasks To Column:" }
                        select {
                            class: "dashboard-form-input",
                            value: "{target_id}",
                            onchange: move |e| {
                                if let Ok(id) = e.value().parse::<i64>() {
                                    target_id.set(id);
                                }
                            },
                            for t in &available_targets {
                                option { key: "{t.id}", value: "{t.id}", "{t.name} ({t.category.as_str()})" }
                            }
                        }
                    }
                }
                div { class: "modal-footer",
                    button {
                        class: "btn-task-cancel",
                        onclick: move |_| on_cancel.call(()),
                        "Cancel"
                    }
                    button {
                        class: "btn-task-save",
                        style: "background: var(--danger); border-color: var(--danger);",
                        disabled: *target_id.read() == 0,
                        onclick: move |_| {
                            let remap_id = *target_id.read();
                            on_confirm.call((status.id, remap_id));
                        },
                        "Remap Tasks & Delete"
                    }
                }
            }
        }
    }
}
