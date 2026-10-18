use super::host::ProjectViewHost;
use crate::backend::app_endpoints::{get_project_board_snapshot, move_task_workflow_status};
use crate::components::projects::InviteProjectMemberModal;
use crate::components::tasks::InlineCreateTaskModal;
use crate::types::{has_perm, Permission, ProjectViewKind, TaskMoveRequest};
use crate::Route;
use dioxus::prelude::*;

/// Full-featured project board view container page.
#[component]
pub fn ProjectBoardPage(project_id: i64, view: Option<ProjectViewKind>) -> Element {
    let mut selected_view = use_signal(|| view.unwrap_or(ProjectViewKind::KanbanClassic));
    let mut show_new_task = use_signal(|| false);
    let mut show_invite_modal = use_signal(|| false);
    let nav = use_navigator();

    // Re-synchronize selected_view if route prop changes
    use_effect(move || {
        if let Some(v) = view {
            selected_view.set(v);
        }
    });

    // Hydration-safe view persistence: read client preference strictly in use_effect post-hydration
    use_effect(move || {
        if view.is_none() {
            spawn(async move {
                let mut eval = document::eval(
                    r#"
                    let pref = localStorage.getItem("taskmaster_view_pref");
                    dioxus.send(pref);
                "#,
                );
                if let Ok(Some(pref)) = eval.recv::<Option<String>>().await {
                    if let Ok(k) = pref.parse::<ProjectViewKind>() {
                        selected_view.set(k);
                    }
                }
            });
        }
    });

    let mut board_resource =
        use_server_future(move || async move { get_project_board_snapshot(project_id).await })?;

    let on_move = move |req: TaskMoveRequest| {
        spawn(async move {
            if move_task_workflow_status(req.task_id, req.target_workflow_status_id)
                .await
                .is_ok()
            {
                board_resource.restart();
            }
        });
    };

    let on_select_view = move |v: ProjectViewKind| {
        selected_view.set(v);
        let nav = nav.clone();
        spawn(async move {
            let _ = document::eval(&format!(
                r#"localStorage.setItem("taskmaster_view_pref", "{}");"#,
                v.as_str()
            ));
            nav.push(Route::ProjectBoardViewPage {
                project_id,
                view: v,
            });
        });
    };

    let res_borrow = board_resource.read();
    match &*res_borrow {
        Some(Err(err)) => rsx! { BoardAccessDenied { err: err.to_string() } },
        Some(Ok(snapshot)) => {
            let snap = snapshot.clone();
            let p_name = snap.project_name.clone();
            let perms = snap.caller_permissions;
            rsx! {
                div { class: "project-board-page",
                    BoardTopBar {
                        project_id,
                        project_name: p_name.clone(),
                        enabled_views: snap.enabled_views.clone(),
                        current_view: selected_view(),
                        can_invite: has_perm(perms, Permission::MANAGE_MEMBERS) || has_perm(perms, Permission::ADMIN),
                        can_manage: has_perm(perms, Permission::ADMIN),
                        can_mutate: has_perm(perms, Permission::EXECUTE),
                        on_select_view,
                        on_toggle_invite: move |_| {
                            let curr = *show_invite_modal.read();
                            show_invite_modal.set(!curr);
                        },
                        on_toggle_new_task: move |_| {
                            let curr = *show_new_task.read();
                            show_new_task.set(!curr);
                        },
                    }

                    if *show_new_task.read() {
                        InlineCreateTaskModal {
                            project_id: Some(project_id),
                            on_close: move |_| show_new_task.set(false),
                            on_created: move |_| {
                                show_new_task.set(false);
                                board_resource.restart();
                            },
                        }
                    }

                    if *show_invite_modal.read() {
                        InviteProjectMemberModal {
                            project_id,
                            project_name: Some(p_name.clone()),
                            on_close: move |_| show_invite_modal.set(false),
                            on_invited: move |_| {
                                show_invite_modal.set(false);
                                board_resource.restart();
                            },
                        }
                    }

                    // Pass fresh snapshot directly instead of caching in a stagnant signal so newly created tasks render immediately
                    ProjectViewHost {
                        view_kind: selected_view,
                        board: snap.clone(),
                        on_move_task: on_move,
                    }
                }
            }
        }
        None => rsx! {
            div { class: "project-board-loading",
                p { "Loading project board..." }
            }
        },
    }
}

/// Access denied message panel when user lacks permissions for this project board.
#[component]
fn BoardAccessDenied(err: String) -> Element {
    rsx! {
        div { class: "project-board-page",
            div {
                class: "dashboard-panel",
                style: "border: 1px solid var(--danger-bg); margin: 24px; background-color: var(--bg-surface); padding: 24px; border-radius: var(--radius-md);",
                h2 { style: "color: var(--danger); margin-bottom: 8px; font-size: var(--font-size-h3);",
                    "Access Denied"
                }
                p { style: "color: var(--text-muted); font-size: var(--font-size-body);",
                    "{err}"
                }
                div { style: "margin-top: 16px;",
                    Link {
                        to: Route::TasksDashboardPage {},
                        class: "btn-back-link",
                        "← Return to My Tasks"
                    }
                }
            }
        }
    }
}

/// Topbar container for project board: title, view-switcher pills, and action buttons.
#[component]
fn BoardTopBar(
    project_id: i64,
    project_name: String,
    enabled_views: Vec<ProjectViewKind>,
    current_view: ProjectViewKind,
    can_invite: bool,
    can_manage: bool,
    can_mutate: bool,
    on_select_view: EventHandler<ProjectViewKind>,
    on_toggle_invite: EventHandler<()>,
    on_toggle_new_task: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "task-view-topbar",
            div { class: "project-title-group",
                h1 { class: "task-view-title", "{project_name}" }
                div { style: "display: flex; gap: 8px; align-items: center; margin-top: 4px;",
                    span { style: "color: var(--text-muted); font-size: var(--font-size-meta);",
                        "Project #{project_id}"
                    }
                }
            }

            BoardViewSwitcher {
                enabled_views,
                current_view,
                on_select: on_select_view,
            }

            div { style: "display: flex; gap: 10px; align-items: center;",
                if can_invite {
                    button {
                        r#type: "button",
                        class: "btn-task-cancel",
                        onclick: move |_| on_toggle_invite.call(()),
                        "👥 Invite Member"
                    }
                }
                if can_manage {
                    Link {
                        to: Route::ProjectManagePage { project_id },
                        class: "btn-task-cancel",
                        style: "text-decoration: none;",
                        "⚙ Manage"
                    }
                }
                if can_mutate {
                    button {
                        r#type: "button",
                        class: "btn-create-task",
                        onclick: move |_| on_toggle_new_task.call(()),
                        span { class: "btn-create-icon", "+" }
                        " New Task"
                    }
                }
            }
        }
    }
}

/// Segmented view switcher button group (`Kanban Classic | Swimlanes | List | Gantt`).
#[component]
fn BoardViewSwitcher(
    enabled_views: Vec<ProjectViewKind>,
    current_view: ProjectViewKind,
    on_select: EventHandler<ProjectViewKind>,
) -> Element {
    rsx! {
        div { class: "view-switcher-bar",
            for v in enabled_views {
                {
                    let is_active = v == current_view;
                    let active_class = if is_active { "active" } else { "" };
                    rsx! {
                        button {
                            key: "{v.as_str()}",
                            r#type: "button",
                            class: "btn-view-tab {active_class}",
                            onclick: move |_| on_select.call(v),
                            span { class: "view-tab-icon", "{v.icon()}" }
                            span { class: "view-tab-label", "{v.label()}" }
                        }
                    }
                }
            }
        }
    }
}
