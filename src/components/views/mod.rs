pub mod board_page;
pub mod gantt;
pub mod host;
pub mod kanban_classic;
pub mod kanban_swimlanes;
pub mod list;
#[cfg(all(test, feature = "server"))]
pub mod tests;

pub use board_page::ProjectBoardPage;
pub use gantt::GanttView;
pub use host::ProjectViewHost;
pub use kanban_classic::KanbanClassicView;
pub use kanban_swimlanes::KanbanSwimlanesView;
pub use list::ListView;
