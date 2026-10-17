use crate::backend::database::get_db;

#[tokio::test]
async fn test_organization_roles_and_deletion_guardrails() {
    let pool = get_db().await;

    // 1. Create test organization
    let org_id: i64 = sqlx::query_scalar(
        "INSERT INTO organization (name, slug, governing_user_id) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind("Test Guardrails Org")
    .bind(format!("guardrails-org-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()))
    .bind(1) // Alice
    .fetch_one(pool)
    .await
    .unwrap();

    // Check trigger auto-seeded 'member' and 'admin'
    let roles: Vec<String> = sqlx::query_scalar(
        "SELECT role FROM organization_role WHERE organization_id = $1 ORDER BY role ASC",
    )
    .bind(org_id)
    .fetch_all(pool)
    .await
    .unwrap();
    assert_eq!(roles, vec!["admin", "member"]);

    // Check trigger protects built-in roles
    let delete_admin_err = sqlx::query(
        "DELETE FROM organization_role WHERE organization_id = $1 AND role = 'admin'",
    )
    .bind(org_id)
    .execute(pool)
    .await;
    assert!(delete_admin_err.is_err(), "Should prevent deleting 'admin' role");

    // Add a second member (Bob)
    sqlx::query("INSERT INTO organization_members (organization_id, user_id, role) VALUES ($1, $2, 'member')")
        .bind(org_id)
        .bind(2)
        .execute(pool)
        .await
        .unwrap();

    // Try deleting organization while Bob is still a member -> should fail!
    let delete_org_err = sqlx::query("DELETE FROM organization WHERE id = $1")
        .bind(org_id)
        .execute(pool)
        .await;
    assert!(delete_org_err.is_err(), "Cannot delete populated organization");

    // Remove Bob
    sqlx::query("DELETE FROM organization_members WHERE organization_id = $1 AND user_id = 2")
        .bind(org_id)
        .execute(pool)
        .await
        .unwrap();

    // Now delete organization should succeed
    let delete_org_ok = sqlx::query("DELETE FROM organization WHERE id = $1")
        .bind(org_id)
        .execute(pool)
        .await;
    assert!(delete_org_ok.is_ok(), "Sole-member organization can be deleted");
}

#[tokio::test]
async fn test_organization_pending_invitations_and_cancellation() {
    use crate::backend::app_endpoints::organizations::get_organization_pending_invitations;
    let pool = get_db().await;

    let org_id: i64 = sqlx::query_scalar(
        "INSERT INTO organization (name, slug, governing_user_id) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind("Invite Test Org")
    .bind(format!("invite-org-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()))
    .bind(1) // Alice
    .fetch_one(pool)
    .await
    .unwrap();

    // Invite Bob (user 2)
    sqlx::query(
        "INSERT INTO organization_invitation (organization_id, invited_user_id, invited_by, role, message)
         VALUES ($1, 2, 1, 'member', 'Welcome to the team!')",
    )
    .bind(org_id)
    .execute(pool)
    .await
    .unwrap();

    let invites = get_organization_pending_invitations(org_id).await.unwrap();
    assert_eq!(invites.len(), 1);
    assert_eq!(invites[0].invited_user_id, 2);
    assert_eq!(invites[0].message, "Welcome to the team!");

    // Cancel invite
    sqlx::query("DELETE FROM organization_invitation WHERE organization_id = $1 AND invited_user_id = 2")
        .bind(org_id)
        .execute(pool)
        .await
        .unwrap();

    let after_cancel = get_organization_pending_invitations(org_id).await.unwrap();
    assert_eq!(after_cancel.len(), 0);

    // Clean up
    let _ = sqlx::query("DELETE FROM organization WHERE id = $1").bind(org_id).execute(pool).await;
}

