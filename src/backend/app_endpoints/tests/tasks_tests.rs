//! # Tasks, Project Association & Cascade Cleanup Test Campaign
//!
//! This test suite verifies task-to-project relational integrity, denormalized metadata resolution
//! (`project_name`), and database cascade cleanup upon project deletion.
//!
//! ## Covered Specifications:
//! - **Project Association**: Verifying tasks correctly track and return their parent `project_id` and `project_name`.
//! - **Batch Fetching**: Verifying `get_project_tasks` filters accurately by project boundary.
//! - **Cascade Deletion**: Ensuring foreign key constraints (`ON DELETE CASCADE`) and cleanup triggers
//!   remove all associated tasks and assignments when a project is removed, leaving no orphaned records.

use crate::backend::app_endpoints::tests::test_helpers::run_as_user;
use crate::backend::app_endpoints::*;
use crate::backend::database::get_db;

/// Verifies that tasks properly link to their parent project, that endpoints return project metadata,
/// and that deleting a project completely and cleanly purges all its child tasks.
///
/// ### Test Execution Flow:
/// 1. **Project Creation**: Creates "Project With Tasks" led by Alice (id: 1).
/// 2. **Task Associations**: Inserts "Task Alpha" and "Task Beta" linked to the project.
/// 3. **Metadata Verification**:
///    - Asserts `get_task(task1_id)` includes correct `project_id` and `"Project With Tasks"` name.
///    - Asserts `get_project_tasks(proj_id)` returns both tasks with resolved project names.
/// 4. **Cascade Deletion**: Deletes the project directly from the database.
/// 5. **Orphan Prevention**: Verifies task count for the project drops to 0, and `get_task` returns errors.
#[tokio::test]
async fn test_project_task_association_and_deletion_cleanup() {
    let pool = get_db().await;

    // Step 1: Create a project led by Alice (user 1)
    let proj_id: i64 = sqlx::query_scalar(
        "INSERT INTO project (name, description, leader_id) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind("Project With Tasks")
    .bind("Testing task-project association and cascade deletion")
    .bind(1) // Alice
    .fetch_one(pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO project_members (project_id, user_id, role) VALUES ($1, $2, 'admin')")
        .bind(proj_id)
        .bind(1)
        .execute(pool)
        .await
        .unwrap();

    // Step 2: Insert two tasks associated with this project
    let task1_id: i64 = sqlx::query_scalar(
        "INSERT INTO task (project_id, title, description, author, status) VALUES ($1, $2, $3, $4, 'suggested') RETURNING id",
    )
    .bind(proj_id)
    .bind("Task Alpha")
    .bind("Description Alpha")
    .bind(1)
    .fetch_one(pool)
    .await
    .unwrap();

    let task2_id: i64 = sqlx::query_scalar(
        "INSERT INTO task (project_id, title, description, author, status) VALUES ($1, $2, $3, $4, 'suggested') RETURNING id",
    )
    .bind(proj_id)
    .bind("Task Beta")
    .bind("Description Beta")
    .bind(1)
    .fetch_one(pool)
    .await
    .unwrap();

    // Step 3: Verify single-task and project-task endpoints return joined project name
    run_as_user(1, move || async move {
        let fetched_task1 = get_task(task1_id).await.unwrap();
        assert_eq!(fetched_task1.project_id, proj_id);
        assert_eq!(fetched_task1.project_name, "Project With Tasks");

        let project_tasks = get_project_tasks(proj_id, None, 10).await.unwrap();
        assert_eq!(project_tasks.len(), 2, "Expected 2 tasks in project");
        assert_eq!(project_tasks[0].project_name, "Project With Tasks");
        assert_eq!(project_tasks[1].project_name, "Project With Tasks");
    })
    .await;

    // Step 4: Delete the parent project; foreign key CASCADE and triggers must clean up child tasks
    sqlx::query("DELETE FROM project WHERE id = $1")
        .bind(proj_id)
        .execute(pool)
        .await
        .unwrap();

    // Step 5: Verify no orphaned tasks remain in the database
    let count_remaining_tasks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM task WHERE project_id = $1")
        .bind(proj_id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(
        count_remaining_tasks, 0,
        "All tasks of deleted project should be cascaded and removed"
    );

    // Endpoints must return not-found errors for the purged tasks
    run_as_user(1, move || async move {
        assert!(get_task(task1_id).await.is_err());
        assert!(get_task(task2_id).await.is_err());
    })
    .await;
}
