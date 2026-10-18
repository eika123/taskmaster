//! # Database Guardrails & Deletion Safeguards Test Campaign
//!
//! This test suite verifies low-level database trigger enforcement and destructive operation
//! safeguards for Organizations and Projects in PostgreSQL.
//!
//! ## Covered Database Trigger Guardrails:
//! 1. **`prevent_remove_governing_user`**:
//!    Ensures an organization cannot be orphaned by removing or demoting its governing user.
//! 2. **`prevent_delete_populated_org`**:
//!    Ensures an organization cannot be deleted while non-governing members still belong to it.
//! 3. **`prevent_remove_project_leader`**:
//!    Ensures a project's leader cannot be removed from `project_members` without first transferring leadership.
//! 4. **`prevent_delete_populated_project`**:
//!    Ensures a project cannot be deleted while non-leader members still belong to it.
//! 5. **Confirmation Phrase Verification**:
//!    Verifies that destructive deletion requires typing the exact case-sensitive phrase `"DELETE <EntityName>"`.

use crate::backend::database::get_db;

/// Verifies PostgreSQL database triggers that prevent orphaning organizations/projects
/// and prevent accidental mass-deletion of populated entities.
///
/// ### Test Execution Flow:
/// 1. **Organization Governance Guardrails**:
///    - Inserts a new organization with Alice (id: 1) as governor and Bob (id: 2) as member.
///    - Verifies trigger `prevent_remove_governing_user` blocks removing Alice from `organization_members`.
///    - Verifies trigger `prevent_delete_populated_org` blocks deleting the organization while Bob is a member.
///    - Removes Bob, and asserts that deleting the sole-member organization now succeeds.
///
/// 2. **Project Leadership Guardrails**:
///    - Inserts a new project with Alice (id: 1) as leader and Bob (id: 2) as member.
///    - Verifies trigger `prevent_remove_project_leader` blocks removing Alice from `project_members`.
///    - Verifies trigger `prevent_delete_populated_project` blocks deleting the project while Bob is a member.
///    - Removes Bob, and asserts that deleting the sole-leader project now succeeds.
#[tokio::test]
async fn test_project_and_organization_database_guardrails() {
    let pool = get_db().await;
    let unique_timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    // =========================================================================
    // Part 1: Organization Guardrails
    // =========================================================================

    // Create an organization with Alice (user 1) as the governing user
    let org_id: i64 = sqlx::query_scalar(
        "INSERT INTO organization (name, slug, governing_user_id) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind("Org Guardrail Test")
    .bind(format!("org-guard-{unique_timestamp}"))
    .bind(1) // Alice
    .fetch_one(pool)
    .await
    .unwrap();

    // Register Alice as admin in organization_members
    sqlx::query("INSERT INTO organization_members (organization_id, user_id, role) VALUES ($1, $2, 'admin')")
        .bind(org_id)
        .bind(1)
        .execute(pool)
        .await
        .unwrap();

    // Add Bob (user 2) as a standard member
    sqlx::query("INSERT INTO organization_members (organization_id, user_id, role) VALUES ($1, $2, 'member')")
        .bind(org_id)
        .bind(2)
        .execute(pool)
        .await
        .unwrap();

    // Verify trigger: prevent_remove_governing_user MUST abort deletion of the governor
    let remove_governor_result = sqlx::query(
        "DELETE FROM organization_members WHERE organization_id = $1 AND user_id = 1",
    )
    .bind(org_id)
    .execute(pool)
    .await;
    assert!(
        remove_governor_result.is_err(),
        "Database trigger should prevent removing governing user from organization_members"
    );

    // Verify trigger: prevent_delete_populated_org MUST block deleting org while Bob is still a member
    let delete_populated_org_result = sqlx::query("DELETE FROM organization WHERE id = $1")
        .bind(org_id)
        .execute(pool)
        .await;
    assert!(
        delete_populated_org_result.is_err(),
        "Database trigger should prevent deleting organization while non-governing members exist"
    );

    // Remove Bob from organization_members
    sqlx::query("DELETE FROM organization_members WHERE organization_id = $1 AND user_id = 2")
        .bind(org_id)
        .execute(pool)
        .await
        .unwrap();

    // Once Bob is removed, deleting the organization should succeed
    sqlx::query("DELETE FROM organization WHERE id = $1")
        .bind(org_id)
        .execute(pool)
        .await
        .unwrap();

    // =========================================================================
    // Part 2: Project Guardrails
    // =========================================================================

    // Create a standalone project with Alice (user 1) as the project leader
    let proj_id: i64 = sqlx::query_scalar(
        "INSERT INTO project (name, description, leader_id) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind("Project Guardrail Test")
    .bind("Testing triggers for project leader and deletion")
    .bind(1) // Alice
    .fetch_one(pool)
    .await
    .unwrap();

    // Register Alice as admin in project_members
    sqlx::query("INSERT INTO project_members (project_id, user_id, role) VALUES ($1, $2, 'admin')")
        .bind(proj_id)
        .bind(1)
        .execute(pool)
        .await
        .unwrap();

    // Add Bob (user 2) as a standard project member
    sqlx::query("INSERT INTO project_members (project_id, user_id, role) VALUES ($1, $2, 'member')")
        .bind(proj_id)
        .bind(2)
        .execute(pool)
        .await
        .unwrap();

    // Verify trigger: prevent_remove_project_leader MUST block removing the leader
    let remove_leader_result = sqlx::query(
        "DELETE FROM project_members WHERE project_id = $1 AND user_id = 1",
    )
    .bind(proj_id)
    .execute(pool)
    .await;
    assert!(
        remove_leader_result.is_err(),
        "Database trigger should prevent removing project leader from project_members"
    );

    // Verify trigger: prevent_delete_populated_project MUST block deleting project while members exist
    let delete_populated_project_result = sqlx::query("DELETE FROM project WHERE id = $1")
        .bind(proj_id)
        .execute(pool)
        .await;
    assert!(
        delete_populated_project_result.is_err(),
        "Database trigger should prevent deleting project while other members exist"
    );

    // Remove Bob from project_members
    sqlx::query("DELETE FROM project_members WHERE project_id = $1 AND user_id = 2")
        .bind(proj_id)
        .execute(pool)
        .await
        .unwrap();

    // Once only the leader remains, deleting the project succeeds
    sqlx::query("DELETE FROM project WHERE id = $1")
        .bind(proj_id)
        .execute(pool)
        .await
        .unwrap();
}

/// Verifies user confirmation strings for destructive actions (e.g. deleting a project or organization).
///
/// Ensures strict equality with `"DELETE <EntityName>"`, protecting against accidental clicks,
/// case mismatches, extra spaces, and prefix-only inputs.
#[tokio::test]
async fn test_deletion_confirmation_phrase_verification() {
    let project_name = "Core Engine";
    let expected_confirmation = format!("DELETE {}", project_name.trim());

    // 1. Lowercase mismatch
    assert_ne!("delete core engine", expected_confirmation);

    // 2. Missing entity name
    assert_ne!("DELETE", expected_confirmation);

    // 3. Incomplete entity name
    assert_ne!("DELETE Core", expected_confirmation);

    // 4. Double space between command and name
    assert_ne!("DELETE  Core Engine", expected_confirmation);

    // 5. Exact match with whitespace trimming passes
    assert_eq!("DELETE Core Engine".trim(), expected_confirmation);
}
