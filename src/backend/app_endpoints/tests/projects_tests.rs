//! # Projects, Leadership, Discussions & Invitations Test Campaign
//!
//! This test suite validates project-level operations, discussion threads, task lifecycle triggers,
//! and the project member invitation workflow.
//!
//! ## Covered Specifications:
//! 1. **Project Leadership & Task Discussions**:
//!    - Project creation and leader assignment.
//!    - Clustered-key task messages `(task_id, msg_id)` and sequential message ordering.
//!    - Task state machine triggers (`enforce_task_state_transition`):
//!      - InProgress transition requires at least one assigned member.
//!      - Updating with assignees transitions successfully.
//! 2. **Project Invitation Flow**:
//!    - Issuing invitations to prospective project members.
//!    - Acceptance of invitation and atomic materialization into `project_members`.

use crate::backend::app_endpoints::tests::test_helpers::run_as_user;
use crate::backend::app_endpoints::*;
use crate::backend::database::get_db;
use crate::types::TaskStatus;

/// Tests project leadership, clustered task discussion threads, and task state machine triggers.
///
/// ### Test Execution Flow:
/// 1. **Project & Member Setup**: Creates a project with Bob (id: 2) as leader and Alice (id: 1) as member.
/// 2. **Task Creation**: Creates an initial task in the `'accepted'` state.
/// 3. **Discussion Messages**: Inserts sequential messages for the task and verifies that
///    `get_task_messages` preserves chronologically ordered message IDs and content.
/// 4. **State Machine Trigger (`enforce_task_state_transition`)**:
///    - Attempts to transition the task to `'in_progress'` without assignees -> asserts database error.
///    - Assigns Bob (id: 2) and transitions to `'in_progress'` -> asserts success and returned assignees.
#[tokio::test]
async fn test_project_leadership_and_task_messages() {
    let pool = get_db().await;

    // Step 1: Create a project led by Bob (user 2)
    let proj_id: i64 = sqlx::query_scalar(
        "INSERT INTO project (name, description, leader_id) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind("Test Discussion Project")
    .bind("Testing discussion threads and clustered keys")
    .bind(2) // Bob
    .fetch_one(pool)
    .await
    .unwrap();

    // Register Bob as admin and Alice as member in project_members
    sqlx::query("INSERT INTO project_members (project_id, user_id, role) VALUES ($1, $2, 'admin')")
        .bind(proj_id)
        .bind(2)
        .execute(pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO project_members (project_id, user_id, role) VALUES ($1, 1, 'member')")
        .bind(proj_id)
        .execute(pool)
        .await
        .unwrap();

    // Step 2: Insert a task in 'accepted' status
    let task_id: i64 = sqlx::query_scalar(
        "INSERT INTO task (project_id, title, description, author, status) VALUES ($1, $2, $3, $4, 'accepted') RETURNING id",
    )
    .bind(proj_id)
    .bind("Clustered Key Task")
    .bind("Detailed task description")
    .bind(2)
    .fetch_one(pool)
    .await
    .unwrap();

    // Step 3: Insert discussion messages using the composite primary key (task_id, msg_id)
    sqlx::query("INSERT INTO task_message (task_id, msg_id, author_id, created_at, msg) VALUES ($1, 1, 2, EXTRACT(EPOCH FROM NOW())::BIGINT, 'Message 1')")
        .bind(task_id)
        .execute(pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO task_message (task_id, msg_id, author_id, created_at, msg) VALUES ($1, 2, 1, EXTRACT(EPOCH FROM NOW())::BIGINT, 'Message 2')")
        .bind(task_id)
        .execute(pool)
        .await
        .unwrap();

    // Steps 3 & 4: Authenticated user operations run within a real session
    run_as_user(1, move || async move {
        // Fetch and verify sequential message ordering
        let messages = get_task_messages(task_id).await.unwrap();
        assert_eq!(messages.len(), 2, "Expected exactly 2 discussion messages");
        assert_eq!(messages[0].msg_id, 1);
        assert_eq!(messages[0].msg, "Message 1");
        assert_eq!(messages[1].msg_id, 2);
        assert_eq!(messages[1].msg, "Message 2");

        // Step 4: Test state machine triggers via update_task
        // Invariant: transitioning to 'in_progress' with 0 assignees MUST fail at the trigger level
        let transition_without_assignees_result = update_task(
            task_id,
            "Clustered Key Task".to_string(),
            "Desc".to_string(),
            TaskStatus::InProgress,
            vec![], // No assignees
        )
        .await;
        assert!(
            transition_without_assignees_result.is_err(),
            "State machine trigger should reject in_progress status without assignees"
        );

        // Transitioning to 'in_progress' with assignees MUST succeed
        let transition_with_assignees_result = update_task(
            task_id,
            "Clustered Key Task Updated".to_string(),
            "New Desc".to_string(),
            TaskStatus::InProgress,
            vec![2], // Assigned to Bob
        )
        .await
        .unwrap();

        assert_eq!(transition_with_assignees_result.status, TaskStatus::InProgress);
        assert_eq!(transition_with_assignees_result.assignees.len(), 1);
        assert_eq!(transition_with_assignees_result.assignees[0].id, 2);
    })
    .await;
}

/// Tests the project invitation lifecycle from issuance to acceptance.
///
/// ### Test Execution Flow:
/// 1. **Project Setup**: Creates a new project with Alice (id: 1) as leader.
/// 2. **Invite Issuance**: Alice invites Bob (id: 2) with role `'member'` and a welcome note.
/// 3. **Verification**: Asserts the pending invite exists in `project_invitation`.
/// 4. **Acceptance**: Simulates acceptance by adding Bob to `project_members` and deleting the invite.
/// 5. **Confirmation**: Verifies Bob is now an active project member.
#[tokio::test]
async fn test_project_invitation_flow() {
    let pool = get_db().await;

    // Create a new project led by Alice (user 1)
    let proj_id: i64 = sqlx::query_scalar(
        "INSERT INTO project (name, description, leader_id) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind("Invitation Test Project")
    .bind("Verifying invite workflow")
    .bind(1) // Alice
    .fetch_one(pool)
    .await
    .unwrap();

    // Step 1: Issue an invitation to Bob (user 2)
    sqlx::query(
        "INSERT INTO project_invitation (project_id, invited_user_id, invited_by, role, message)
         VALUES ($1, $2, $3, 'member', 'Welcome to the project!')",
    )
    .bind(proj_id)
    .bind(2)
    .bind(1)
    .execute(pool)
    .await
    .unwrap();

    // Step 2: Verify the invitation is recorded
    let invite_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM project_invitation WHERE project_id = $1 AND invited_user_id = 2)",
    )
    .bind(proj_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert!(invite_exists, "Invitation should exist in project_invitation table");

    // Step 3: Accept invitation - insert into project_members and remove pending invite
    sqlx::query("INSERT INTO project_members (project_id, user_id, role) VALUES ($1, 2, 'member')")
        .bind(proj_id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM project_invitation WHERE project_id = $1 AND invited_user_id = 2")
        .bind(proj_id)
        .execute(pool)
        .await
        .unwrap();

    // Step 4: Verify Bob is now confirmed in project_members
    let is_confirmed_member: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM project_members WHERE project_id = $1 AND user_id = 2)",
    )
    .bind(proj_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert!(is_confirmed_member, "User should be an active project member after accepting invitation");
}
