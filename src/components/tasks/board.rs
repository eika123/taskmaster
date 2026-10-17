use dioxus::prelude::*;
use crate::components::column_selector::ColumnSelector;
use crate::backend::app_endpoints::{
    assign_task, delete_task, get_my_tasks, get_project, get_project_tasks,
    get_users, set_task_status, unassign_task,
};
use crate::types::TaskStatus;
use super::column::TaskColumn;
use super::modals::{InlineCreateTaskModal, InlineInviteModal};
use crate::components::dashboard::ConfirmDeleteModal;

#[component]
pub fn TaskView(project_id: Option<i64>) -> Element {
    let mut selected_statuses = use_signal(|| vec![
        TaskStatus::Suggested,
        TaskStatus::Accepted,
        TaskStatus::Assigned,
        TaskStatus::InProgress,
        TaskStatus::Finished,
    ]);

    let pid = project_id;
    let mut tasks_resource = use_server_future(move || async move {
        if let Some(p) = pid {
            get_project_tasks(p, None, 100).await
        } else {
            get_my_tasks(None, 100).await
        }
    })?;

    let project_resource = use_server_future(move || async move {
        if let Some(p) = pid {
            get_project(p).await.ok()
        } else {
            None
        }
    })?;

    let users_resource = use_server_future(|| async move {
        get_users().await
    })?;

    let mut task_list = use_signal(|| {
        match &*tasks_resource.read() {
            Some(Ok(list)) => list.clone(),
            _ => Vec::new(),
        }
    });

    let mut all_users = use_signal(|| {
        match &*users_resource.read() {
            Some(Ok(list)) => list.clone(),
            _ => Vec::new(),
        }
    });

    use_effect(move || {
        if let Some(Ok(list)) = &*tasks_resource.read() {
            task_list.set(list.clone());
        }
    });

    use_effect(move || {
        if let Some(Ok(list)) = &*users_resource.read() {
            all_users.set(list.clone());
        }
    });

    let mut show_new_task = use_signal(|| false);
    let mut show_invite_modal = use_signal(|| false);
    let mut show_delete_modal = use_signal(|| false);
    let mut is_deleting_project = use_signal(|| false);
    let mut delete_proj_err = use_signal(|| Option::<String>::None);
    let auth = use_context::<crate::components::AuthState>();
    let current_user_id = auth.user.read().as_ref().map(|u| u.id);
    let nav = use_navigator();

    let on_dismiss_handler = move |task_id: i64| {
        task_list.with_mut(|tasks| {
            tasks.retain(|t| t.id != task_id);
        });
        spawn(async move {
            let _ = delete_task(task_id).await;
            tasks_resource.restart();
        });
    };

    let on_status_change_handler = move |(task_id, new_status): (u64, TaskStatus)| {
        task_list.with_mut(|tasks| {
            if let Some(t) = tasks.iter_mut().find(|t| t.id == task_id as i64) {
                t.status = new_status;
            }
        });
        spawn(async move {
            let _ = set_task_status(task_id, new_status).await;
            tasks_resource.restart();
        });
    };

    let on_assign_handler = move |(task_id, user_id): (i64, i64)| {
        spawn(async move {
            if assign_task(task_id, user_id).await.is_ok() {
                tasks_resource.restart();
            }
        });
    };

    let on_unassign_handler = move |(task_id, user_id): (i64, i64)| {
        spawn(async move {
            if unassign_task(task_id, user_id).await.is_ok() {
                tasks_resource.restart();
            }
        });
    };

    let active_statuses = selected_statuses();
    let current_tasks = task_list();
    let current_users = all_users();

    let project_info = match &*project_resource.read() {
        Some(Some(proj)) => Some(proj.clone()),
        _ => None,
    };

    rsx! {
        div { class: "task-view",
            div { class: "task-view-topbar",
                div {
                    if let Some(proj) = &project_info {
                        h1 { class: "task-view-title", "{proj.name}" }
                        div { style: "display: flex; gap: 8px; align-items: center; margin-top: 4px;",
                            span { style: "color: var(--text-muted); font-size: var(--font-size-meta);",
                                "Led by "
                                strong { "{proj.leader_name}" }
                            }
                            if let Some(org_name) = &proj.owner_org_name {
                                span { class: "task-detail-status status-accepted badge-sm",
                                    "{org_name}"
                                }
                            } else {
                                span { class: "task-detail-status status-suggested badge-sm",
                                    "Standalone"
                                }
                            }
                        }
                    } else {
                        h1 { class: "task-view-title", "My Tasks" }
                        p { style: "margin: 4px 0 0; color: var(--text-muted); font-size: var(--font-size-body-sm);",
                            "Tasks assigned to or created by you across all projects"
                        }
                    }
                }

                div { style: "display: flex; gap: 10px; align-items: center;",
                    if pid.is_some() {
                        button {
                            r#type: "button",
                            class: "btn-task-cancel",
                            onclick: move |_| {
                                let curr = *show_invite_modal.read();
                                show_invite_modal.set(!curr);
                            },
                            "👥 Invite Member"
                        }
                    }
                    if pid.is_some() && project_info.as_ref().map(|p| p.leader_id) == current_user_id {
                        button {
                            r#type: "button",
                            class: "btn-task-cancel",
                            style: "color: var(--danger); border-color: var(--danger-bg);",
                            onclick: move |_| {
                                show_delete_modal.set(true);
                            },
                            "🗑 Delete Project"
                        }
                    }
                    button {
                        r#type: "button",
                        class: "btn-create-task",
                        onclick: move |_| {
                            let curr = *show_new_task.read();
                            show_new_task.set(!curr);
                        },
                        span { class: "btn-create-icon", "+" }
                        " New Task"
                    }
                }
            }

            if *show_delete_modal.read() {
                if let (Some(target_pid), Some(pinfo)) = (pid, &project_info) {
                    {
                        let pname = pinfo.name.clone();
                        rsx! {
                            ConfirmDeleteModal {
                                entity_type: "project".to_string(),
                                entity_name: pname,
                                on_close: move |_| {
                                    show_delete_modal.set(false);
                                    delete_proj_err.set(None);
                                },
                                on_confirm: move |confirmation: String| {
                                    is_deleting_project.set(true);
                                    delete_proj_err.set(None);
                                    let nav = nav.clone();
                                    spawn(async move {
                                        match crate::backend::app_endpoints::delete_project(target_pid, confirmation).await {
                                            Ok(_) => {
                                                is_deleting_project.set(false);
                                                show_delete_modal.set(false);
                                                nav.push(crate::Route::DashboardPage {});
                                            }
                                            Err(e) => {
                                                is_deleting_project.set(false);
                                                delete_proj_err.set(Some(e.to_string()));
                                            }
                                        }
                                    });
                                },
                                is_deleting: *is_deleting_project.read(),
                                error: (*delete_proj_err.read()).clone(),
                            }
                        }
                    }
                }
            }

            if *show_new_task.read() {
                InlineCreateTaskModal {
                    project_id: pid,
                    on_close: move |_| show_new_task.set(false),
                    on_created: move |_| {
                        show_new_task.set(false);
                        tasks_resource.restart();
                    },
                }
            }

            if *show_invite_modal.read() && pid.is_some() {
                InlineInviteModal {
                    project_id: pid.unwrap(),
                    current_users: current_users.clone(),
                    on_close: move |_| show_invite_modal.set(false),
                }
            }

            // Interactive Column Selector
            ColumnSelector {
                selected_statuses: active_statuses.clone(),
                on_toggle: move |status: TaskStatus| {
                    let mut list = selected_statuses();
                    if let Some(pos) = list.iter().position(|&s| s == status) {
                        list.remove(pos);
                    } else {
                        list.push(status);
                    }
                    selected_statuses.set(list);
                },
                on_select_all: move |_| {
                    selected_statuses
                        .set(
                            vec![
                                TaskStatus::Suggested,
                                TaskStatus::Accepted,
                                TaskStatus::Assigned,
                                TaskStatus::InProgress,
                                TaskStatus::Finished,
                            ],
                        );
                },
                on_clear: move |_| {
                    selected_statuses.set(Vec::new());
                },
            }

            // Columns Container: Side-by-side slideable board
            if active_statuses.is_empty() {
                div { class: "no-columns-notice",
                    p { "No columns selected. Click one or more status pills above to view tasks." }
                }
            } else {
                div { class: "task-board-scroll-container",
                    div { class: "task-columns-row",
                        for status in active_statuses {
                            TaskColumn {
                                key: "{status.as_str()}",
                                status,
                                tasks: current_tasks.clone(),
                                all_users: current_users.clone(),
                                on_dismiss: on_dismiss_handler,
                                on_status_change: on_status_change_handler,
                                on_assign: on_assign_handler,
                                on_unassign: on_unassign_handler,
                            }
                        }
                    }
                }
            }
        }
    }
}

