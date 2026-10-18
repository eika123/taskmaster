use super::gantt::GanttView;
use super::kanban_classic::KanbanClassicView;
use super::kanban_swimlanes::KanbanSwimlanesView;
use super::list::ListView;
use crate::types::{BoardSnapshot, ProjectViewKind, TaskMoveRequest};
use dioxus::prelude::*;

/// Pluggable project view registry host component.
///
/// Dispatches rendering to the selected [`ProjectViewKind`] projection component
/// over the immutable [`BoardSnapshot`] contract.
#[component]
pub fn ProjectViewHost(
    view_kind: ReadSignal<ProjectViewKind>,
    board: ReadSignal<BoardSnapshot>,
    on_move_task: EventHandler<TaskMoveRequest>,
) -> Element {
    rsx! {
        div { class: "project-view-host",
            match view_kind() {
                ProjectViewKind::KanbanClassic => rsx! { KanbanClassicView { board, on_move_task } },
                ProjectViewKind::KanbanSwimlanes => rsx! { KanbanSwimlanesView { board, on_move_task } },
                ProjectViewKind::List => rsx! { ListView { board, on_move_task } },
                ProjectViewKind::Gantt => rsx! { GanttView { board } },
            }
        }
    }
}
