use crate::backend::database::get_db;

#[tokio::test]
async fn test_project_and_organization_database_guardrails() {
    let pool = get_db().await;

    // 1. Organization guardrails
    let org_id: i64 = sqlx::query_scalar(
        "INSERT INTO organization (name, slug, governing_user_id) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind("Org Guardrail Test")
    .bind(format!("org-guard-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()))
    .bind(1) // Alice
    .fetch_one(pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO organization_members (organization_id, user_id, role) VALUES ($1, $2, 'admin')")
        .bind(org_id)
        .bind(1)
        .execute(pool)
        .await
        .unwrap();

    // Add second member Bob
    sqlx::query("INSERT INTO organization_members (organization_id, user_id, role) VALUES ($1, $2, 'member')")
        .bind(org_id)
        .bind(2)
        .execute(pool)
        .await
        .unwrap();

    // Verify trigger prevent_remove_governing_user blocks removing governing user (Alice)
    let remove_gov_err = sqlx::query("DELETE FROM organization_members WHERE organization_id = $1 AND user_id = 1")
        .bind(org_id)
        .execute(pool)
        .await;
    assert!(remove_gov_err.is_err(), "Cannot remove governing user from organization_members");

    // Verify trigger prevent_delete_populated_org blocks deleting org while Bob is member
    let del_org_err = sqlx::query("DELETE FROM organization WHERE id = $1")
        .bind(org_id)
        .execute(pool)
        .await;
    assert!(del_org_err.is_err(), "Cannot delete populated organization");

    // Removing Bob succeeds
    sqlx::query("DELETE FROM organization_members WHERE organization_id = $1 AND user_id = 2")
        .bind(org_id)
        .execute(pool)
        .await
        .unwrap();

    // Deleting org now succeeds (only governing user remains)
    sqlx::query("DELETE FROM organization WHERE id = $1")
        .bind(org_id)
        .execute(pool)
        .await
        .unwrap();

    // 2. Project guardrails
    let proj_id: i64 = sqlx::query_scalar(
        "INSERT INTO project (name, description, leader_id) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind("Project Guardrail Test")
    .bind("Testing triggers for project leader and deletion")
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

    // Add second member Bob
    sqlx::query("INSERT INTO project_members (project_id, user_id, role) VALUES ($1, $2, 'member')")
        .bind(proj_id)
        .bind(2)
        .execute(pool)
        .await
        .unwrap();

    // Verify trigger prevent_remove_project_leader blocks removing project leader (Alice)
    let remove_leader_err = sqlx::query("DELETE FROM project_members WHERE project_id = $1 AND user_id = 1")
        .bind(proj_id)
        .execute(pool)
        .await;
    assert!(remove_leader_err.is_err(), "Cannot remove project leader from project_members");

    // Verify trigger prevent_delete_populated_project blocks deleting project while Bob is member
    let del_proj_err = sqlx::query("DELETE FROM project WHERE id = $1")
        .bind(proj_id)
        .execute(pool)
        .await;
    assert!(del_proj_err.is_err(), "Cannot delete populated project");

    // Removing Bob succeeds
    sqlx::query("DELETE FROM project_members WHERE project_id = $1 AND user_id = 2")
        .bind(proj_id)
        .execute(pool)
        .await
        .unwrap();

    // Deleting project now succeeds (only leader remains)
    sqlx::query("DELETE FROM project WHERE id = $1")
        .bind(proj_id)
        .execute(pool)
        .await
        .unwrap();
}

