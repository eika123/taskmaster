//! # Milestone M1: Workflow Engine (Expand Phase) & TC1 Test Campaign
//!
//! Validates:
//! 1. Automatic seeding of the 5-state default workflow on project creation.
//! 2. Dynamic semantic flags: `allows_assignment` blocks assigning initial tasks.
//! 3. Dynamic semantic flags: `requires_assignee` enforces assignee requirements for in-progress tasks.
//! 4. Soft WIP limit behavior: database writes succeed while `wip_exceeded: true` is reported.
//! 5. Composite foreign key cross-project isolation: assigning a task to another project's status is rejected by PostgreSQL.
//! 6. Status deletion safety: `ON DELETE RESTRICT` prevents orphaning tasks on column deletion.
//! 7. Role-gated workflow transitions: users lacking required permission bits receive errors on restricted moves.

use crate::backend::app_endpoints::projects::create_project;
use crate::backend::app_endpoints::tasks::*;
use crate::backend::app_endpoints::tests::test_helpers::run_as_user;
use crate::backend::database::get_db;
use crate::types::{BadgeVariant, Permission, StatusCategory};

/// 1. Verifies that creating a project automatically seeds the 5 default workflow statuses
/// with appropriate categories, positions, and semantic flags.
#[tokio::test]
async fn test_auto_seed_default_workflow_statuses_on_project_creation() {
    let pool = get_db().await;

    // Create a new project led by Alice (user 1)
    let proj_id: i64 = sqlx::query_scalar(
        "INSERT INTO project (name, description, leader_id) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind("Workflow Auto-Seed Project")
    .bind("Testing trigger auto-seeding")
    .bind(1)
    .fetch_one(pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO project_members (project_id, user_id, role, permissions_mask) VALUES ($1, $2, 'admin', $3)")
        .bind(proj_id)
        .bind(1)
        .bind(Permission::ADMIN_DEFAULT.as_i64())
        .execute(pool)
        .await
        .unwrap();

    run_as_user(1, move || async move {
        let statuses = get_project_workflow_statuses(proj_id).await.unwrap();
        assert_eq!(statuses.len(), 5, "Expected 5 auto-seeded statuses");

        // Verify order and properties
        assert_eq!(statuses[0].name, "Suggested");
        assert_eq!(statuses[0].category, StatusCategory::Todo);
        assert_eq!(statuses[0].badge_variant, BadgeVariant::Default);
        assert_eq!(statuses[0].position, 1000);
        assert!(statuses[0].is_initial);
        assert!(!statuses[0].allows_assignment);
        assert!(!statuses[0].requires_assignee);

        assert_eq!(statuses[1].name, "Accepted");
        assert_eq!(statuses[1].category, StatusCategory::Todo);
        assert_eq!(statuses[1].badge_variant, BadgeVariant::Info);
        assert_eq!(statuses[1].position, 2000);
        assert!(!statuses[1].is_initial);
        assert!(statuses[1].allows_assignment);
        assert!(!statuses[1].requires_assignee);

        assert_eq!(statuses[2].name, "Assigned");
        assert_eq!(statuses[2].category, StatusCategory::Todo);
        assert_eq!(statuses[2].badge_variant, BadgeVariant::Purple);
        assert_eq!(statuses[2].position, 3000);
        assert!(!statuses[2].is_initial);
        assert!(statuses[2].allows_assignment);
        assert!(!statuses[2].requires_assignee);

        assert_eq!(statuses[3].name, "InProgress");
        assert_eq!(statuses[3].category, StatusCategory::InProgress);
        assert_eq!(statuses[3].badge_variant, BadgeVariant::Warning);
        assert_eq!(statuses[3].position, 4000);
        assert!(!statuses[3].is_initial);
        assert!(statuses[3].allows_assignment);
        assert!(statuses[3].requires_assignee);

        assert_eq!(statuses[4].name, "Finished");
        assert_eq!(statuses[4].category, StatusCategory::Done);
        assert_eq!(statuses[4].badge_variant, BadgeVariant::Success);
        assert_eq!(statuses[4].position, 5000);
        assert!(!statuses[4].is_initial);
        assert!(statuses[4].allows_assignment);
        assert!(!statuses[4].requires_assignee);

        // Exactly one initial status constraint
        let initial_count = statuses.iter().filter(|s| s.is_initial).count();
        assert_eq!(initial_count, 1);
    })
    .await;
}

/// 2. Verifies that the semantic flag `allows_assignment = false` blocks assigning tasks in initial status,
/// and that transitioning to a status with `allows_assignment = true` permits assignment.
#[tokio::test]
async fn test_semantic_flag_allows_assignment_blocks_assignment_for_initial_status() {
    let pool = get_db().await;

    let proj_id: i64 = sqlx::query_scalar(
        "INSERT INTO project (name, description, leader_id) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind("Assignment Flag Project")
    .bind("Testing allows_assignment semantic flag")
    .bind(1)
    .fetch_one(pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO project_members (project_id, user_id, role, permissions_mask) VALUES ($1, 1, 'admin', $2), ($1, 2, 'member', $3)")
        .bind(proj_id)
        .bind(Permission::ADMIN_DEFAULT.as_i64())
        .bind(Permission::MEMBER_DEFAULT.as_i64())
        .execute(pool)
        .await
        .unwrap();

    let task_id: i64 = sqlx::query_scalar(
        "INSERT INTO task (project_id, title, description, author) VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(proj_id)
    .bind("Initial Task")
    .bind("Cannot be assigned while allows_assignment is false")
    .bind(1)
    .fetch_one(pool)
    .await
    .unwrap();

    // Verify task automatically received workflow_status_id via trigger
    let ws_id: Option<i64> =
        sqlx::query_scalar("SELECT workflow_status_id FROM task WHERE id = $1")
            .bind(task_id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert!(
        ws_id.is_some(),
        "Trigger must auto-populate workflow_status_id"
    );

    // Attempting to assign while allows_assignment = false must fail
    run_as_user(1, move || async move {
        let assign_result = assign_task(task_id, 2).await;
        assert!(
            assign_result.is_err(),
            "Assignment must be rejected when allows_assignment is false"
        );

        // Transition task to Accepted (allows_assignment = true)
        let statuses = get_project_workflow_statuses(proj_id).await.unwrap();
        let accepted_id = statuses.iter().find(|s| s.name == "Accepted").unwrap().id;
        move_task_workflow_status(task_id, accepted_id)
            .await
            .unwrap();

        // Assignment must now succeed
        let assign_after = assign_task(task_id, 2).await;
        assert!(
            assign_after.is_ok(),
            "Assignment must succeed when allows_assignment is true"
        );
    })
    .await;
}

/// 3. Verifies that the semantic flag `requires_assignee = true` enforces that at least one assignee exists,
/// and prevents removing the last assignee while in that status.
#[tokio::test]
async fn test_semantic_flag_requires_assignee_enforces_assignment_for_in_progress() {
    let pool = get_db().await;

    let proj_id: i64 = sqlx::query_scalar(
        "INSERT INTO project (name, description, leader_id) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind("Requires Assignee Project")
    .bind("Testing requires_assignee semantic flag")
    .bind(1)
    .fetch_one(pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO project_members (project_id, user_id, role, permissions_mask) VALUES ($1, 1, 'admin', $2), ($1, 2, 'member', $3)")
        .bind(proj_id)
        .bind(Permission::ADMIN_DEFAULT.as_i64())
        .bind(Permission::MEMBER_DEFAULT.as_i64())
        .execute(pool)
        .await
        .unwrap();

    let task_id: i64 = sqlx::query_scalar(
        "INSERT INTO task (project_id, title, description, author, workflow_status_id) VALUES ($1, $2, $3, $4, (SELECT id FROM project_workflow_status WHERE project_id = $1 AND name = 'Accepted')) RETURNING id",
    )
    .bind(proj_id)
    .bind("Unassigned Task")
    .bind("Testing in_progress enforcement")
    .bind(1)
    .fetch_one(pool)
    .await
    .unwrap();

    run_as_user(1, move || async move {
        let statuses = get_project_workflow_statuses(proj_id).await.unwrap();
        let in_progress_id = statuses.iter().find(|s| s.name == "InProgress").unwrap().id;

        // 1. Moving to InProgress without assignees must fail
        let move_fail = move_task_workflow_status(task_id, in_progress_id).await;
        assert!(
            move_fail.is_err(),
            "Must reject InProgress without assignee"
        );

        // 2. Assign Bob (user 2)
        assign_task(task_id, 2).await.unwrap();

        // 3. Moving to InProgress with assignee must succeed
        let move_ok = move_task_workflow_status(task_id, in_progress_id).await;
        assert!(move_ok.is_ok(), "Must allow InProgress with assignee");

        // 4. Attempting to unassign the last assignee while in InProgress must fail
        let unassign_fail = unassign_task(task_id, 2).await;
        assert!(
            unassign_fail.is_err(),
            "Must prevent removing last assignee from in_progress task"
        );
    })
    .await;
}

/// 4. Verifies soft WIP limit behavior:
/// - Inserting tasks beyond `wip_limit` NEVER fails the database insert.
/// - `WorkflowStatusDto.wip_exceeded` is computed dynamically as `task_count > wip_limit`.
#[tokio::test]
async fn test_soft_wip_limit_allows_writes_and_flags_wip_exceeded() {
    let pool = get_db().await;

    let proj_id: i64 = sqlx::query_scalar(
        "INSERT INTO project (name, description, leader_id) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind("Soft WIP Limit Project")
    .bind("Testing soft WIP limits")
    .bind(1)
    .fetch_one(pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO project_members (project_id, user_id, role, permissions_mask) VALUES ($1, 1, 'admin', $2)")
        .bind(proj_id)
        .bind(Permission::ADMIN_DEFAULT.as_i64())
        .execute(pool)
        .await
        .unwrap();

    // Set wip_limit = 2 on 'Accepted' status
    sqlx::query("UPDATE project_workflow_status SET wip_limit = 2 WHERE project_id = $1 AND name = 'Accepted'")
        .bind(proj_id)
        .execute(pool)
        .await
        .unwrap();

    let accepted_id: i64 = sqlx::query_scalar(
        "SELECT id FROM project_workflow_status WHERE project_id = $1 AND name = 'Accepted'",
    )
    .bind(proj_id)
    .fetch_one(pool)
    .await
    .unwrap();

    // Insert task 1 into Accepted
    sqlx::query("INSERT INTO task (project_id, title, author, workflow_status_id) VALUES ($1, 'Task 1', 1, $2)")
        .bind(proj_id)
        .bind(accepted_id)
        .execute(pool)
        .await
        .unwrap();

    // Insert task 2 into Accepted
    sqlx::query("INSERT INTO task (project_id, title, author, workflow_status_id) VALUES ($1, 'Task 2', 1, $2)")
        .bind(proj_id)
        .bind(accepted_id)
        .execute(pool)
        .await
        .unwrap();

    run_as_user(1, move || async move {
        let statuses = get_project_workflow_statuses(proj_id).await.unwrap();
        let accepted = statuses.iter().find(|s| s.id == accepted_id).unwrap();
        assert_eq!(accepted.task_count, 2);
        assert!(!accepted.wip_exceeded, "2 tasks out of wip_limit 2 is not exceeded");

        // Insert task 3 (exceeding wip_limit 2): database write MUST SUCCEED (Soft WIP limit)
        let insert3 = sqlx::query("INSERT INTO task (project_id, title, author, workflow_status_id) VALUES ($1, 'Task 3', 1, $2)")
            .bind(proj_id)
            .bind(accepted_id)
            .execute(get_db().await)
            .await;
        assert!(insert3.is_ok(), "Soft WIP limit must NEVER block database writes");

        let statuses_after = get_project_workflow_statuses(proj_id).await.unwrap();
        let accepted_after = statuses_after.iter().find(|s| s.id == accepted_id).unwrap();
        assert_eq!(accepted_after.task_count, 3);
        assert!(accepted_after.wip_exceeded, "3 tasks out of wip_limit 2 must report wip_exceeded = true");
    })
    .await;
}

/// 5. Verifies composite foreign key cross-project isolation:
/// Assigning a task in Project A to a `workflow_status_id` in Project B is rejected by PostgreSQL.
#[tokio::test]
async fn test_composite_fk_rejects_cross_project_workflow_status() {
    let pool = get_db().await;

    // Create Project A
    let proj_a: i64 = sqlx::query_scalar(
        "INSERT INTO project (name, description, leader_id) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind("Project Alpha")
    .bind("First isolation boundary")
    .bind(1)
    .fetch_one(pool)
    .await
    .unwrap();

    // Create Project B
    let proj_b: i64 = sqlx::query_scalar(
        "INSERT INTO project (name, description, leader_id) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind("Project Beta")
    .bind("Second isolation boundary")
    .bind(1)
    .fetch_one(pool)
    .await
    .unwrap();

    // Get a status belonging to Project B
    let status_b: i64 = sqlx::query_scalar(
        "SELECT id FROM project_workflow_status WHERE project_id = $1 AND name = 'Accepted'",
    )
    .bind(proj_b)
    .fetch_one(pool)
    .await
    .unwrap();

    // Attempting to insert a task in Project A pointing to Project B's status must fail with FK violation
    let cross_project_insert = sqlx::query(
        "INSERT INTO task (project_id, title, author, workflow_status_id) VALUES ($1, 'Cross-Project Task', 1, $2)",
    )
    .bind(proj_a)
    .bind(status_b)
    .execute(pool)
    .await;

    assert!(
        cross_project_insert.is_err(),
        "Composite FK (project_id, workflow_status_id) must reject cross-project status assignment"
    );
}

/// 6. Verifies status deletion safety:
/// Deleting a workflow status that has tasks pointing to it is rejected with ON DELETE RESTRICT.
#[tokio::test]
async fn test_status_deletion_blocked_by_on_delete_restrict_when_tasks_assigned() {
    let pool = get_db().await;

    let proj_id: i64 = sqlx::query_scalar(
        "INSERT INTO project (name, description, leader_id) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind("Status Delete Safety Project")
    .bind("Testing ON DELETE RESTRICT on status deletion")
    .bind(1)
    .fetch_one(pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO project_members (project_id, user_id, role, permissions_mask) VALUES ($1, 1, 'admin', $2)")
        .bind(proj_id)
        .bind(Permission::ADMIN_DEFAULT.as_i64())
        .execute(pool)
        .await
        .unwrap();

    let accepted_id: i64 = sqlx::query_scalar(
        "SELECT id FROM project_workflow_status WHERE project_id = $1 AND name = 'Accepted'",
    )
    .bind(proj_id)
    .fetch_one(pool)
    .await
    .unwrap();

    // Insert task into Accepted status
    sqlx::query("INSERT INTO task (project_id, title, author, workflow_status_id) VALUES ($1, 'Anchored Task', 1, $2)")
        .bind(proj_id)
        .bind(accepted_id)
        .execute(pool)
        .await
        .unwrap();

    // Attempting to delete the status while the task points to it must fail with ON DELETE RESTRICT
    let delete_result = sqlx::query("DELETE FROM project_workflow_status WHERE id = $1")
        .bind(accepted_id)
        .execute(pool)
        .await;

    assert!(
        delete_result.is_err(),
        "ON DELETE RESTRICT must prevent deleting a workflow status that has assigned tasks"
    );
}

/// 7. Verifies role-gated workflow transitions:
/// - Moving across an unrestricted workflow succeeds for regular members.
/// - Once transitions are configured in `project_workflow_transition`, transitions require specified permissions.
#[tokio::test]
async fn test_role_gated_workflow_transition() {
    let pool = get_db().await;

    let proj_id: i64 = sqlx::query_scalar(
        "INSERT INTO project (name, description, leader_id) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind("Gated Transitions Project")
    .bind("Testing role-gated transitions")
    .bind(1) // Alice is leader (Admin)
    .fetch_one(pool)
    .await
    .unwrap();

    // Alice is leader (Admin), Bob is member with EXECUTE (4) but lacking ADMIN (32)
    sqlx::query("INSERT INTO project_members (project_id, user_id, role, permissions_mask) VALUES ($1, 1, 'admin', $2), ($1, 2, 'member', $3)")
        .bind(proj_id)
        .bind(Permission::ADMIN_DEFAULT.as_i64())
        .bind(Permission::EXECUTE.as_i64())
        .execute(pool)
        .await
        .unwrap();

    let accepted_id: i64 = sqlx::query_scalar(
        "SELECT id FROM project_workflow_status WHERE project_id = $1 AND name = 'Accepted'",
    )
    .bind(proj_id)
    .fetch_one(pool)
    .await
    .unwrap();

    let finished_id: i64 = sqlx::query_scalar(
        "SELECT id FROM project_workflow_status WHERE project_id = $1 AND name = 'Finished'",
    )
    .bind(proj_id)
    .fetch_one(pool)
    .await
    .unwrap();

    // Configure role-gated transition: Accepted -> Finished requires Permission::ADMIN (32)
    sqlx::query(
        "INSERT INTO project_workflow_transition (project_id, from_status_id, to_status_id, required_permission) VALUES ($1, $2, $3, $4)",
    )
    .bind(proj_id)
    .bind(accepted_id)
    .bind(finished_id)
    .bind(Permission::ADMIN.as_i64())
    .execute(pool)
    .await
    .unwrap();

    // Create a task in Accepted status
    let task_id: i64 = sqlx::query_scalar(
        "INSERT INTO task (project_id, title, author, workflow_status_id) VALUES ($1, 'Gate Task', 1, $2) RETURNING id",
    )
    .bind(proj_id)
    .bind(accepted_id)
    .fetch_one(pool)
    .await
    .unwrap();

    // 1. Bob (user 2, lacking ADMIN) attempts to move task Accepted -> Finished
    run_as_user(2, move || async move {
        let bob_move = move_task_workflow_status(task_id, finished_id).await;
        assert!(
            bob_move.is_err(),
            "Bob lacking Permission::ADMIN must be blocked from moving to Finished"
        );
    })
    .await;

    // 2. Alice (user 1, Admin) moves task Accepted -> Finished
    run_as_user(1, move || async move {
        let alice_move = move_task_workflow_status(task_id, finished_id).await;
        assert!(
            alice_move.is_ok(),
            "Alice with Permission::ADMIN must successfully execute gated transition"
        );
    })
    .await;
}

/// 8. Verifies get_project_board_snapshot retrieves the canonical BoardSnapshot
/// with ordered statuses, computed task counts, caller permissions, and tasks.
#[tokio::test]
async fn test_get_project_board_snapshot_contract() {
    let pool = get_db().await;

    let proj_id: i64 = sqlx::query_scalar(
        "INSERT INTO project (name, description, leader_id) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind("Snapshot Test Project")
    .bind("Testing BoardSnapshot endpoint")
    .bind(1)
    .fetch_one(pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO project_members (project_id, user_id, role, permissions_mask) VALUES ($1, $2, 'admin', $3)")
        .bind(proj_id)
        .bind(1)
        .bind(Permission::ADMIN_DEFAULT.as_i64())
        .execute(pool)
        .await
        .unwrap();

    // Fetch initial status id
    let initial_status_id: i64 = sqlx::query_scalar(
        "SELECT id FROM project_workflow_status WHERE project_id = $1 AND is_initial = TRUE",
    )
    .bind(proj_id)
    .fetch_one(pool)
    .await
    .unwrap();

    // Create 2 tasks in this project
    sqlx::query(
        "INSERT INTO task (project_id, title, author, workflow_status_id) VALUES
         ($1, 'Snapshot Task 1', 1, $2),
         ($1, 'Snapshot Task 2', 1, $2)",
    )
    .bind(proj_id)
    .bind(initial_status_id)
    .execute(pool)
    .await
    .unwrap();

    // Run as Alice (User 1)
    run_as_user(1, move || async move {
        let snapshot = get_project_board_snapshot(proj_id).await.unwrap();

        assert_eq!(snapshot.project_id, proj_id);
        assert_eq!(snapshot.project_name, "Snapshot Test Project");
        assert_eq!(
            snapshot.default_view,
            crate::types::ProjectViewKind::KanbanClassic
        );
        assert!(snapshot
            .enabled_views
            .contains(&crate::types::ProjectViewKind::KanbanClassic));
        assert!(snapshot
            .enabled_views
            .contains(&crate::types::ProjectViewKind::KanbanSwimlanes));
        assert!(snapshot
            .enabled_views
            .contains(&crate::types::ProjectViewKind::List));
        assert!(snapshot
            .enabled_views
            .contains(&crate::types::ProjectViewKind::Gantt));

        assert_eq!(snapshot.statuses.len(), 5);
        assert_eq!(snapshot.tasks.len(), 2);

        // Verify task count on initial status
        let initial_ws = snapshot.find_status(initial_status_id).unwrap();
        assert_eq!(initial_ws.task_count, 2);
    })
    .await;

    // Run as non-member (User 3) - should be denied
    run_as_user(3, move || async move {
        let res = get_project_board_snapshot(proj_id).await;
        assert!(
            res.is_err(),
            "Non-member must be denied access to BoardSnapshot"
        );
    })
    .await;
}

/// 9. Verifies move_task_workflow_status transitions a task to target status column
/// and synchronizes the legacy status column via triggers.
#[tokio::test]
async fn test_move_task_workflow_status_end_to_end() {
    let pool = get_db().await;

    let proj_id: i64 = sqlx::query_scalar(
        "INSERT INTO project (name, description, leader_id) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind("Move Task Test Project")
    .bind("Testing move_task_workflow_status endpoint")
    .bind(1)
    .fetch_one(pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO project_members (project_id, user_id, role, permissions_mask) VALUES ($1, $2, 'admin', $3)")
        .bind(proj_id)
        .bind(1)
        .bind(Permission::ADMIN_DEFAULT.as_i64())
        .execute(pool)
        .await
        .unwrap();

    let accepted_id: i64 = sqlx::query_scalar(
        "SELECT id FROM project_workflow_status WHERE project_id = $1 AND name = 'Accepted'",
    )
    .bind(proj_id)
    .fetch_one(pool)
    .await
    .unwrap();

    let finished_id: i64 = sqlx::query_scalar(
        "SELECT id FROM project_workflow_status WHERE project_id = $1 AND name = 'Finished'",
    )
    .bind(proj_id)
    .fetch_one(pool)
    .await
    .unwrap();

    let task_id: i64 = sqlx::query_scalar(
        "INSERT INTO task (project_id, title, author, workflow_status_id) VALUES ($1, 'Move Target Task', 1, $2) RETURNING id",
    )
    .bind(proj_id)
    .bind(accepted_id)
    .fetch_one(pool)
    .await
    .unwrap();

    run_as_user(1, move || async move {
        let updated_task = move_task_workflow_status(task_id, finished_id)
            .await
            .unwrap();
        assert_eq!(updated_task.workflow_status_id, finished_id);
        assert_eq!(updated_task.workflow_status_name.as_deref(), Some("Finished"));
    })
    .await;
}

/// 10. Verifies get_workflow_templates returns starter templates (Kanban, Guild Raid, Scrum, Legacy)
#[tokio::test]
async fn test_workflow_starter_templates_availability() {
    let templates = get_workflow_templates().await.unwrap();
    assert!(templates.len() >= 4, "Expected at least 4 starter templates");

    let template_ids: Vec<&str> = templates.iter().map(|t| t.id.as_str()).collect();
    assert!(template_ids.contains(&"classic_kanban"), "Missing Classic Kanban template");
    assert!(template_ids.contains(&"guild_raid_prep"), "Missing Guild Raid Prep template");
    assert!(template_ids.contains(&"software_sprint"), "Missing Software Sprint template");
    assert!(template_ids.contains(&"legacy_flow"), "Missing Legacy Flow template");

    let kanban = templates.iter().find(|t| t.id == "classic_kanban").unwrap();
    assert_eq!(kanban.statuses.len(), 4);
    assert!(kanban.statuses.iter().any(|s| s.is_initial));
    assert!(kanban.statuses.iter().any(|s| s.requires_assignee));
}

/// 11. Verifies apply_workflow_template transitions existing project tasks into the new initial status
/// and replaces existing workflow columns with the template's statuses.
#[tokio::test]
async fn test_apply_workflow_template_to_project() {
    let pool = get_db().await;

    let proj_id: i64 = sqlx::query_scalar(
        "INSERT INTO project (name, description, leader_id) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind("Template Apply Project")
    .bind("Testing apply_workflow_template")
    .bind(1)
    .fetch_one(pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO project_members (project_id, user_id, role, permissions_mask) VALUES ($1, 1, 'admin', $2)")
        .bind(proj_id)
        .bind(Permission::ADMIN_DEFAULT.as_i64())
        .execute(pool)
        .await
        .unwrap();

    // Insert task in initial default status
    let task_id: i64 = sqlx::query_scalar(
        "INSERT INTO task (project_id, title, author) VALUES ($1, 'Template Task', 1) RETURNING id",
    )
    .bind(proj_id)
    .fetch_one(pool)
    .await
    .unwrap();

    run_as_user(1, move || async move {
        // Apply "guild_raid_prep" template
        let new_statuses = apply_workflow_template(proj_id, "guild_raid_prep".to_string())
            .await
            .unwrap();

        assert_eq!(new_statuses.len(), 5);
        assert_eq!(new_statuses[0].name, "Signups");
        assert_eq!(new_statuses[1].name, "Roster Assigned");
        assert_eq!(new_statuses[2].name, "Consumables & Gear");
        assert_eq!(new_statuses[3].name, "Ready for Raid");
        assert_eq!(new_statuses[4].name, "Raid Complete");

        // Verify that the task was safely remapped to the new initial status
        let initial_status = new_statuses.iter().find(|s| s.is_initial).unwrap();
        let task = get_task(task_id).await.unwrap();
        assert_eq!(task.workflow_status_id, initial_status.id);
        assert_eq!(task.workflow_status_name.as_deref(), Some("Signups"));
    })
    .await;
}

/// 12. Verifies creating and reordering custom workflow statuses.
#[tokio::test]
async fn test_create_and_reorder_workflow_statuses() {
    let pool = get_db().await;

    let proj_id: i64 = sqlx::query_scalar(
        "INSERT INTO project (name, description, leader_id) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind("Reorder Status Project")
    .bind("Testing status creation and reordering")
    .bind(1)
    .fetch_one(pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO project_members (project_id, user_id, role, permissions_mask) VALUES ($1, 1, 'admin', $2)")
        .bind(proj_id)
        .bind(Permission::ADMIN_DEFAULT.as_i64())
        .execute(pool)
        .await
        .unwrap();

    run_as_user(1, move || async move {
        // Create a new custom status: "QA Review"
        let qa_status = create_workflow_status(
            proj_id,
            "QA Review".to_string(),
            StatusCategory::Review,
            BadgeVariant::Purple,
            Some(3),
            true,
            false,
        )
        .await
        .unwrap();

        assert_eq!(qa_status.name, "QA Review");
        assert_eq!(qa_status.category, StatusCategory::Review);
        assert_eq!(qa_status.badge_variant, BadgeVariant::Purple);
        assert_eq!(qa_status.wip_limit, Some(3));

        let mut statuses = get_project_workflow_statuses(proj_id).await.unwrap();
        assert_eq!(statuses.len(), 6);

        // Reorder statuses in reverse order
        statuses.reverse();
        let reordered_ids: Vec<i64> = statuses.iter().map(|s| s.id).collect();
        reorder_workflow_statuses(proj_id, reordered_ids.clone())
            .await
            .unwrap();

        let after_reorder = get_project_workflow_statuses(proj_id).await.unwrap();
        let after_ids: Vec<i64> = after_reorder.iter().map(|s| s.id).collect();
        assert_eq!(after_ids, reordered_ids, "Statuses must match requested reorder order");
    })
    .await;
}

/// 13. Verifies delete_workflow_status guardrail:
/// - Fails if tasks are currently in the status and no remap target is given.
/// - Succeeds and moves tasks when a valid remap target is supplied.
#[tokio::test]
async fn test_delete_workflow_status_with_remap_guardrail() {
    let pool = get_db().await;

    let proj_id: i64 = sqlx::query_scalar(
        "INSERT INTO project (name, description, leader_id) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind("Delete Remap Project")
    .bind("Testing delete remap guardrail")
    .bind(1)
    .fetch_one(pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO project_members (project_id, user_id, role, permissions_mask) VALUES ($1, 1, 'admin', $2)")
        .bind(proj_id)
        .bind(Permission::ADMIN_DEFAULT.as_i64())
        .execute(pool)
        .await
        .unwrap();

    let accepted_id: i64 = sqlx::query_scalar(
        "SELECT id FROM project_workflow_status WHERE project_id = $1 AND name = 'Accepted'",
    )
    .bind(proj_id)
    .fetch_one(pool)
    .await
    .unwrap();

    let suggested_id: i64 = sqlx::query_scalar(
        "SELECT id FROM project_workflow_status WHERE project_id = $1 AND name = 'Suggested'",
    )
    .bind(proj_id)
    .fetch_one(pool)
    .await
    .unwrap();

    // Insert a task into 'Accepted'
    let task_id: i64 = sqlx::query_scalar(
        "INSERT INTO task (project_id, title, author, workflow_status_id) VALUES ($1, 'Remap Task', 1, $2) RETURNING id",
    )
    .bind(proj_id)
    .bind(accepted_id)
    .fetch_one(pool)
    .await
    .unwrap();

    run_as_user(1, move || async move {
        // Attempting to delete 'Accepted' without remap must fail because it contains 1 task
        let delete_fail = delete_workflow_status(accepted_id, None).await;
        assert!(
            delete_fail.is_err(),
            "Must reject deleting status with tasks without remap target"
        );

        // Deleting with remap to 'Suggested' must succeed
        let delete_ok = delete_workflow_status(accepted_id, Some(suggested_id)).await;
        assert!(delete_ok.is_ok(), "Must succeed when remap target is provided");

        // Verify task is now in 'Suggested'
        let task = get_task(task_id).await.unwrap();
        assert_eq!(task.workflow_status_id, suggested_id);

        // Verify 'Accepted' is completely removed from project statuses
        let remaining_statuses = get_project_workflow_statuses(proj_id).await.unwrap();
        assert!(!remaining_statuses.iter().any(|s| s.id == accepted_id));
    })
    .await;
}

#[tokio::test]
async fn test_new_project_task_creation_and_my_workflow_statuses_visibility() {
    let _pool = crate::backend::database::get_db().await;

    run_as_user(1, || async move {
        // 1. Create a brand new project
        let proj = create_project(
            "Auto Tasks Sync Project".to_string(),
            "Testing instant task visibility".to_string(),
            None,
            Some("classic_kanban".to_string()),
        )
        .await
        .expect("Project creation must succeed");

        let proj_id = proj.id;

        // 2. Add tasks via direct project ID and via fallback 0 ID
        suggest_task_for_project(
            proj_id,
            "Direct Project Task".to_string(),
            "Direct task details".to_string(),
        )
        .await
        .expect("Direct task suggestion must succeed");

        suggest_task_for_project(
            0,
            "Auto Resolved Project Task".to_string(),
            "Auto task details".to_string(),
        )
        .await
        .expect("Auto-resolved task suggestion must succeed");

        // 3. Fetch workflow statuses for user's views
        let my_statuses = get_my_workflow_statuses()
            .await
            .expect("get_my_workflow_statuses must succeed");
        assert!(
            !my_statuses.is_empty(),
            "User must have workflow statuses available"
        );
        let has_backlog = my_statuses.iter().any(|s| s.name == "Backlog");
        assert!(has_backlog, "My statuses must contain 'Backlog'");

        // 4. Fetch my tasks and verify instant presence with matching status metadata
        let my_tasks = get_my_tasks(None, 100)
            .await
            .expect("get_my_tasks must succeed");
        let direct_task = my_tasks
            .iter()
            .find(|t| t.title == "Direct Project Task")
            .expect("Direct Project Task must be present in user's task view");
        assert_eq!(direct_task.project_id, proj_id);
        assert_eq!(
            direct_task.workflow_status_name.as_deref(),
            Some("Backlog")
        );

        let auto_task = my_tasks
            .iter()
            .find(|t| t.title == "Auto Resolved Project Task")
            .expect("Auto Resolved Task must be present in user's task view");
        assert_eq!(auto_task.project_id, proj_id);
        assert_eq!(
            auto_task.workflow_status_name.as_deref(),
            Some("Backlog")
        );
    })
    .await;
}
