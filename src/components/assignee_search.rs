use dioxus::prelude::*;
use crate::types::User;

#[component]
pub fn AssigneeBadgeList(
    assignees: Vec<User>,
    task_title: String,
    can_mutate: bool,
    on_unassign: EventHandler<i64>,
) -> Element {
    rsx! {
        div { class: "assignees-list",
            span { class: "assignees-label", "Assignees:" }
            if assignees.is_empty() {
                span { class: "assignee-placeholder", "Unassigned" }
            } else {
                for user in assignees {
                    {
                        let user_id = user.id;
                        let user_name = user.name.clone();
                        let title = task_title.clone();
                        rsx! {
                            span { key: "{user_id}", class: "assignee-badge",
                                "{user_name}"
                                if can_mutate {
                                    button {
                                        r#type: "button",
                                        class: "btn-remove-assignee",
                                        aria_label: "Remove {user_name} from task {title}",
                                        onclick: move |_| on_unassign.call(user_id),
                                        "✕"
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

#[component]
pub fn AssigneeSearchDropdown(
    all_users: Vec<User>,
    current_assignees: Vec<User>,
    task_title: String,
    on_assign: Option<EventHandler<i64>>,
) -> Element {
    let mut search_query = use_signal(String::new);
    let mut is_dropdown_open = use_signal(|| false);

    let query_str = search_query.read().to_lowercase();
    let filtered_candidates: Vec<User> = all_users
        .into_iter()
        .filter(|u| {
            !current_assignees.iter().any(|a| a.id == u.id)
                && (query_str.is_empty()
                    || u.name.to_lowercase().contains(&query_str)
                    || u.email.to_lowercase().contains(&query_str))
        })
        .collect();

    rsx! {
        div { class: "assignee-search-container",
            input {
                r#type: "text",
                class: "assignee-search-input",
                placeholder: "+ Add assignee...",
                value: "{search_query}",
                aria_label: "Search user to assign to {task_title}",
                oninput: move |evt| {
                    *search_query.write() = evt.value();
                    is_dropdown_open.set(true);
                },
                onfocus: move |_| is_dropdown_open.set(true),
            }

            if *is_dropdown_open.read() && !filtered_candidates.is_empty() {
                ul { class: "assignee-dropdown", role: "listbox",
                    for candidate in filtered_candidates {
                        {
                            let candidate_id = candidate.id;
                            let candidate_name = candidate.name.clone();
                            let candidate_email = candidate.email.clone();
                            rsx! {
                                li {
                                    key: "{candidate_id}",
                                    class: "assignee-dropdown-item",
                                    role: "option",
                                    onclick: move |_| {
                                        if let Some(cb) = on_assign {
                                            cb.call(candidate_id);
                                        }
                                        search_query.write().clear();
                                        is_dropdown_open.set(false);
                                    },
                                    div { class: "dropdown-user-name", "{candidate_name}" }
                                    div { class: "dropdown-user-email", "{candidate_email}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn TaskAssigneesSection(
    assignees: Vec<User>,
    all_users: Vec<User>,
    task_title: String,
    can_mutate: bool,
    on_assign: EventHandler<i64>,
    on_unassign: EventHandler<i64>,
) -> Element {
    rsx! {
        div { class: "task-card-assignees",
            AssigneeBadgeList {
                assignees: assignees.clone(),
                task_title: task_title.clone(),
                can_mutate,
                on_unassign: move |uid| on_unassign.call(uid),
            }
            if can_mutate {
                AssigneeSearchDropdown {
                    all_users,
                    current_assignees: assignees,
                    task_title,
                    on_assign: move |uid| on_assign.call(uid),
                }
            }
        }
    }
}
