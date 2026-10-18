pub mod assignee_search;
pub mod auth;
pub mod column_selector;
pub mod landing;
pub mod new_task;
pub mod status_select;
pub mod task_detail;
pub mod tasks;
pub mod register;
pub mod login;
pub mod navbar;
pub mod dashboard;
pub mod organizations;
pub mod projects;

pub use auth::*;
pub use landing::*;
pub use new_task::*;
pub use task_detail::*;
pub use tasks::*;
pub use register::*;
pub use login::*;
pub use navbar::*;
pub use dashboard::*;
pub use organizations::*;
pub use projects::*;

#[cfg(test)]
mod ssr_tests;
