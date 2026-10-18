pub mod assignee_search;
pub mod auth;
pub mod column_selector;
pub mod dashboard;
pub mod landing;
pub mod login;
pub mod navbar;
pub mod new_task;
pub mod organizations;
pub mod projects;
pub mod register;
pub mod status_select;
pub mod task_detail;
pub mod tasks;
pub mod views;

pub use auth::*;
pub use dashboard::*;
pub use landing::*;
pub use login::*;
pub use navbar::*;
pub use new_task::*;
pub use organizations::*;
pub use projects::*;
pub use register::*;
pub use task_detail::*;
pub use tasks::*;
pub use views::*;

#[cfg(all(test, feature = "server"))]
mod ssr_tests;
