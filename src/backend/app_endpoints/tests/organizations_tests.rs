//! # Organizations, Roles & Pending Invitations Test Campaign
//!
//! This test suite validates organization lifecycle rules, role definition protections,
//! deletion restrictions on populated organizations, and the pending invitation subsystem.
//!
//! ## Covered Specifications:
//! 1. **Organization Roles & Guardrails**:
//!    - Automatic trigger seeding of default roles (`'admin'` and `'member'`).
//!    - Immutability of built-in organization roles (blocking deletion of `'admin'`).
//!    - Blocking organization deletion while non-governing members remain.
//!    - Successful deletion once all non-governing members have departed.
//! 2. **Organization Invitations**:
//!    - Issuing member invitations with custom invitation notes.
//!    - Listing pending invitations via `get_organization_pending_invitations`.
//!    - Revoking/canceling pending invitations and asserting removal.

use crate::backend::database::get_db;

/// Verifies organization role auto-seeding, built-in role protection triggers,
/// and deletion blockers when non-governing members are present.
///
/// ### Test Execution Flow:
/// 1. **Auto-seeding Roles**: Creates an organization and verifies database triggers automatically
///    seed `'admin'` and `'member'` into `organization_role`.
/// 2. **Built-in Role Protection**: Attempts to delete the `'admin'` role and asserts the trigger rejects it.
/// 3. **Populated Org Deletion Protection**: Adds Bob as a member and asserts `DELETE FROM organization` fails.
/// 4. **Safe Deletion**: Removes Bob and asserts that deleting the sole-governor organization succeeds.
#[tokio::test]
async fn test_organization_roles_and_deletion_guardrails() {
    let pool = get_db().await;
    let unique_timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    // Step 1: Create test organization with Alice (user 1) as governor
    let org_id: i64 = sqlx::query_scalar(
        "INSERT INTO organization (name, slug, governing_user_id) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind("Test Guardrails Org")
    .bind(format!("guardrails-org-{unique_timestamp}"))
    .bind(1) // Alice
    .fetch_one(pool)
    .await
    .unwrap();

    // Verify trigger auto-seeded default 'admin' and 'member' roles
    let seeded_roles: Vec<String> =
        sqlx::query_scalar("SELECT role FROM organization_role WHERE organization_id = $1;")
            .bind(org_id)
            .fetch_all(pool)
            .await
            .unwrap();

    // check "admin" and "member" are in seeded_roles
    assert!(
        seeded_roles.iter().any(|r| r == "admin"),
        "missing admin role"
    );
    assert!(
        seeded_roles.iter().any(|r| r == "member"),
        "missing member role"
    );

    // Step 2: Verify trigger protects built-in roles against deletion
    let delete_admin_result =
        sqlx::query("DELETE FROM organization_role WHERE organization_id = $1 AND role = 'admin'")
            .bind(org_id)
            .execute(pool)
            .await;
    assert!(
        delete_admin_result.is_err(),
        "Database trigger should prevent deleting built-in 'admin' role"
    );

    // Step 3: Add Bob (user 2) as a member
    sqlx::query("INSERT INTO organization_members (organization_id, user_id, role) VALUES ($1, $2, 'member')")
        .bind(org_id)
        .bind(2)
        .execute(pool)
        .await
        .unwrap();

    // Deleting organization while Bob is a member MUST fail
    let delete_populated_org_result = sqlx::query("DELETE FROM organization WHERE id = $1")
        .bind(org_id)
        .execute(pool)
        .await;
    assert!(
        delete_populated_org_result.is_err(),
        "Database trigger should prevent deleting organization while populated with other members"
    );

    // Step 4: Remove Bob from members
    sqlx::query("DELETE FROM organization_members WHERE organization_id = $1 AND user_id = 2")
        .bind(org_id)
        .execute(pool)
        .await
        .unwrap();

    // Deletion should now succeed with only the governing user remaining
    let delete_org_success = sqlx::query("DELETE FROM organization WHERE id = $1")
        .bind(org_id)
        .execute(pool)
        .await;
    assert!(
        delete_org_success.is_ok(),
        "Sole-member organization should be successfully deleted"
    );
}

/// Verifies organization invitation creation, pending list retrieval, and cancellation.
///
/// ### Test Execution Flow:
/// 1. **Organization Setup**: Creates an organization with Alice as governor.
/// 2. **Invite Issuance**: Issues an invitation to Bob (user 2) with a custom message.
/// 3. **Listing Invitations**: Calls `get_organization_pending_invitations` and asserts Bob's invite is returned.
/// 4. **Cancellation**: Deletes the invitation and asserts the pending list is now empty.
/// 5. **Teardown**: Cleans up the test organization.
#[tokio::test]
async fn test_organization_pending_invitations_and_cancellation() {
    use crate::backend::app_endpoints::organizations::get_organization_pending_invitations;
    use crate::backend::app_endpoints::tests::test_helpers::run_as_user;
    let pool = get_db().await;
    let unique_timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    // Create test organization
    let org_id: i64 = sqlx::query_scalar(
        "INSERT INTO organization (name, slug, governing_user_id) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind("Invite Test Org")
    .bind(format!("invite-org-{unique_timestamp}"))
    .bind(1) // Alice
    .fetch_one(pool)
    .await
    .unwrap();

    // Step 1: Invite Bob (user 2) to the organization
    sqlx::query(
        "INSERT INTO organization_invitation (organization_id, invited_user_id, invited_by, role, message)
         VALUES ($1, 2, 1, 'member', 'Welcome to the team!')",
    )
    .bind(org_id)
    .execute(pool)
    .await
    .unwrap();

    // Step 2: Fetch pending invitations and verify contents
    run_as_user(1, move || async move {
        let pending_invitations = get_organization_pending_invitations(org_id).await.unwrap();
        assert_eq!(
            pending_invitations.len(),
            1,
            "Expected 1 pending invitation"
        );
        assert_eq!(pending_invitations[0].invited_user_id, 2);
        assert_eq!(pending_invitations[0].message, "Welcome to the team!");
    })
    .await;

    // Step 3: Cancel the invitation
    sqlx::query(
        "DELETE FROM organization_invitation WHERE organization_id = $1 AND invited_user_id = 2",
    )
    .bind(org_id)
    .execute(pool)
    .await
    .unwrap();

    // Step 4: Verify pending list is now empty
    run_as_user(1, move || async move {
        let after_cancellation = get_organization_pending_invitations(org_id).await.unwrap();
        assert_eq!(
            after_cancellation.len(),
            0,
            "Pending invitations should be empty after cancellation"
        );
    })
    .await;

    // Clean up
    let _ = sqlx::query("DELETE FROM organization WHERE id = $1")
        .bind(org_id)
        .execute(pool)
        .await;
}
