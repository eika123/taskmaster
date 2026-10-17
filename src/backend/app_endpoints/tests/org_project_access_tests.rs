use crate::backend::database::get_db;

#[tokio::test]
async fn test_multi_role_potato_people_and_additive_privileges() {
    let pool = get_db().await;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();

    // 1. Create Organization (Acme Oslo, gov: Alice=1)
    let org_id: i64 = sqlx::query_scalar(
        "INSERT INTO organization (name, slug, governing_user_id) VALUES ($1, $2, 1) RETURNING id",
    )
    .bind("Acme Oslo")
    .bind(format!("acme-oslo-{}", now))
    .fetch_one(pool)
    .await
    .unwrap();

    // Add custom roles 'teacher' and 'parent'
    sqlx::query("INSERT INTO organization_role (organization_id, role) VALUES ($1, 'teacher'), ($1, 'parent')")
        .bind(org_id)
        .execute(pool)
        .await
        .unwrap();

    // Add Kari (user 3, Charlie Brown) to the organization with initial role 'parent'
    sqlx::query("INSERT INTO organization_members (organization_id, user_id, role) VALUES ($1, 3, 'parent')")
        .bind(org_id)
        .execute(pool)
        .await
        .unwrap();

    // Also assign Kari second role 'teacher' ("Potato person" wearing multiple hats!)
    sqlx::query("INSERT INTO organization_member_roles (organization_id, user_id, role) VALUES ($1, 3, 'teacher')")
        .bind(org_id)
        .execute(pool)
        .await
        .unwrap();

    // Verify Kari has both roles in organization_member_roles
    let roles: Vec<String> = sqlx::query_scalar(
        "SELECT role FROM organization_member_roles WHERE organization_id = $1 AND user_id = 3 ORDER BY role ASC",
    )
    .bind(org_id)
    .fetch_all(pool)
    .await
    .unwrap();
    assert_eq!(roles, vec!["parent", "teacher"]);

    // 2. Create Project: Class 4B (led by Alice = 1)
    let proj_id: i64 = sqlx::query_scalar(
        "INSERT INTO project (name, description, leader_id) VALUES ($1, 'Class 4B', 1) RETURNING id",
    )
    .bind(format!("Class 4B - {}", now))
    .fetch_one(pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO project_members (project_id, user_id, role, permissions_mask) VALUES ($1, 1, 'admin', 63)")
        .bind(proj_id)
        .execute(pool)
        .await
        .unwrap();

    // Affiliate Acme Oslo with Class 4B (max ceiling: admin)
    sqlx::query(
        "INSERT INTO project_organizations (project_id, organization_id, relationship_type, max_granted_role)
         VALUES ($1, $2, 'partner', 'admin')",
    )
    .bind(proj_id)
    .bind(org_id)
    .execute(pool)
    .await
    .unwrap();

    // 3. Map 'parent' -> 'viewer' (mask 1)
    sqlx::query(
        "INSERT INTO project_org_role_access (project_id, organization_id, org_role, project_role)
         VALUES ($1, $2, 'parent', 'viewer')",
    )
    .bind(proj_id)
    .bind(org_id)
    .execute(pool)
    .await
    .unwrap();

    // Kari should immediately appear in project_members as viewer (mask 1)
    let (kari_role, kari_mask): (String, i64) = sqlx::query_as(
        "SELECT role, permissions_mask FROM project_members WHERE project_id = $1 AND user_id = 3",
    )
    .bind(proj_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(kari_role, "viewer");
    assert_eq!(kari_mask, 1);

    // 4. Map 'teacher' -> 'admin' (mask 63)
    sqlx::query(
        "INSERT INTO project_org_role_access (project_id, organization_id, org_role, project_role)
         VALUES ($1, $2, 'teacher', 'admin')",
    )
    .bind(proj_id)
    .bind(org_id)
    .execute(pool)
    .await
    .unwrap();

    // Additive privilege union: Kari's materialized role is upgraded to 'admin' (mask 63 = 1 | 63)
    let (upgraded_role, upgraded_mask): (String, i64) = sqlx::query_as(
        "SELECT role, permissions_mask FROM project_members WHERE project_id = $1 AND user_id = 3",
    )
    .bind(proj_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(upgraded_role, "admin");
    assert_eq!(upgraded_mask, 63);

    // Verify Kari holds 2 grants in project_member_grants
    let grant_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM project_member_grants WHERE project_id = $1 AND user_id = 3",
    )
    .bind(proj_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(grant_count, 2);

    // 5. Test Hat Switching: Kari pins her hat to 'org:<org_id>:parent'
    let parent_grant_key = format!("org:{}:parent", org_id);
    sqlx::query(
        "UPDATE project_members
         SET active_grant_key = $1,
             permissions_mask = (SELECT permissions_mask FROM project_role_definition WHERE role = 'viewer'),
             role = 'viewer'
         WHERE project_id = $2 AND user_id = 3",
    )
    .bind(&parent_grant_key)
    .bind(proj_id)
    .execute(pool)
    .await
    .unwrap();

    let (pinned_role, pinned_mask, pinned_key): (String, i64, Option<String>) = sqlx::query_as(
        "SELECT role, permissions_mask, active_grant_key FROM project_members WHERE project_id = $1 AND user_id = 3",
    )
    .bind(proj_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(pinned_role, "viewer");
    assert_eq!(pinned_mask, 1);
    assert_eq!(pinned_key, Some(parent_grant_key));

    // Reset Hat to union
    sqlx::query(
        "UPDATE project_members
         SET active_grant_key = NULL,
             permissions_mask = 63,
             role = 'admin'
         WHERE project_id = $1 AND user_id = 3",
    )
    .bind(proj_id)
    .execute(pool)
    .await
    .unwrap();

    // 6. Role Revocation: Kari stops teaching (revoking 'teacher' role in Acme Oslo)
    sqlx::query("DELETE FROM organization_member_roles WHERE organization_id = $1 AND user_id = 3 AND role = 'teacher'")
        .bind(org_id)
        .execute(pool)
        .await
        .unwrap();

    // Kari is NOT kicked out of the class! Her effective role steps down to 'viewer'
    let (stepdown_role, stepdown_mask): (String, i64) = sqlx::query_as(
        "SELECT role, permissions_mask FROM project_members WHERE project_id = $1 AND user_id = 3",
    )
    .bind(proj_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(stepdown_role, "viewer");
    assert_eq!(stepdown_mask, 1);

    // 7. Organization leaves project: Acme Oslo leaves Class 4B
    sqlx::query("DELETE FROM project_organizations WHERE project_id = $1 AND organization_id = $2")
        .bind(proj_id)
        .bind(org_id)
        .execute(pool)
        .await
        .unwrap();

    // Now Kari has 0 grants on Class 4B, so she is automatically removed from project_members
    let remaining_members: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM project_members WHERE project_id = $1 AND user_id = 3",
    )
    .bind(proj_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(remaining_members, 0);

    // Project leader Alice (direct member) remains untouched
    let alice_exists: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM project_members WHERE project_id = $1 AND user_id = 1",
    )
    .bind(proj_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(alice_exists, 1);
}

#[tokio::test]
async fn test_privilege_ceiling_and_viewer_restrictions() {
    let pool = get_db().await;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();

    // Create Org: External Audit Corp
    let org_id: i64 = sqlx::query_scalar(
        "INSERT INTO organization (name, slug, governing_user_id) VALUES ($1, $2, 1) RETURNING id",
    )
    .bind("External Audit Corp")
    .bind(format!("audit-corp-{}", now))
    .fetch_one(pool)
    .await
    .unwrap();

    // Create Project
    let proj_id: i64 = sqlx::query_scalar(
        "INSERT INTO project (name, description, leader_id) VALUES ($1, 'Security Review', 1) RETURNING id",
    )
    .bind(format!("Security Review - {}", now))
    .fetch_one(pool)
    .await
    .unwrap();

    // Affiliate with ceiling: 'viewer' (mask 1)
    sqlx::query(
        "INSERT INTO project_organizations (project_id, organization_id, relationship_type, max_granted_role)
         VALUES ($1, $2, 'auditor', 'viewer')",
    )
    .bind(proj_id)
    .bind(org_id)
    .execute(pool)
    .await
    .unwrap();

    // 1. Attempting to grant 'admin' (63) exceeds ceiling 'viewer' (1) -> MUST FAIL!
    let grant_admin = sqlx::query(
        "INSERT INTO project_org_role_access (project_id, organization_id, org_role, project_role)
         VALUES ($1, $2, 'admin', 'admin')",
    )
    .bind(proj_id)
    .bind(org_id)
    .execute(pool)
    .await;
    assert!(grant_admin.is_err(), "Should block granting role exceeding ceiling");

    // 2. Attempting to grant 'member' (15) exceeds ceiling 'viewer' (1) -> MUST FAIL!
    let grant_member = sqlx::query(
        "INSERT INTO project_org_role_access (project_id, organization_id, org_role, project_role)
         VALUES ($1, $2, 'admin', 'member')",
    )
    .bind(proj_id)
    .bind(org_id)
    .execute(pool)
    .await;
    assert!(grant_member.is_err(), "Should block granting member role exceeding viewer ceiling");

    // 3. Granting 'viewer' (1) matches ceiling -> SUCCEEDS!
    let grant_viewer = sqlx::query(
        "INSERT INTO project_org_role_access (project_id, organization_id, org_role, project_role)
         VALUES ($1, $2, 'admin', 'viewer')",
    )
    .bind(proj_id)
    .bind(org_id)
    .execute(pool)
    .await;
    assert!(grant_viewer.is_ok());

    // Add Diana (user 4) to Audit Corp as admin -> materialized into project as viewer
    sqlx::query("INSERT INTO organization_members (organization_id, user_id, role) VALUES ($1, 4, 'admin')")
        .bind(org_id)
        .execute(pool)
        .await
        .unwrap();

    let diana_role: String = sqlx::query_scalar(
        "SELECT role FROM project_members WHERE project_id = $1 AND user_id = 4",
    )
    .bind(proj_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(diana_role, "viewer");

    // 4. Verify viewer restriction: Diana cannot author tasks
    let diana_task = sqlx::query(
        "INSERT INTO task (project_id, title, author, status) VALUES ($1, 'Auditor Task', 4, 'suggested')",
    )
    .bind(proj_id)
    .execute(pool)
    .await;
    assert!(diana_task.is_err(), "Viewer should not be able to author tasks");
}

#[tokio::test]
async fn test_project_leadership_cascade_guardrail() {
    let pool = get_db().await;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();

    // Create Org
    let org_id: i64 = sqlx::query_scalar(
        "INSERT INTO organization (name, slug, governing_user_id) VALUES ($1, $2, 1) RETURNING id",
    )
    .bind("Engineering Guild")
    .bind(format!("eng-guild-{}", now))
    .fetch_one(pool)
    .await
    .unwrap();

    // Add Evan (user 5) to Org as member
    sqlx::query("INSERT INTO organization_members (organization_id, user_id, role) VALUES ($1, 5, 'member')")
        .bind(org_id)
        .execute(pool)
        .await
        .unwrap();

    // Create Project led by Evan (user 5)
    let proj_id: i64 = sqlx::query_scalar(
        "INSERT INTO project (name, description, leader_id) VALUES ($1, 'Guild Project', 5) RETURNING id",
    )
    .bind(format!("Guild Project - {}", now))
    .fetch_one(pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO project_members (project_id, user_id, role, permissions_mask) VALUES ($1, 5, 'admin', 63)")
        .bind(proj_id)
        .execute(pool)
        .await
        .unwrap();

    // Affiliate Org as partner/admin
    sqlx::query(
        "INSERT INTO project_organizations (project_id, organization_id, relationship_type, max_granted_role)
         VALUES ($1, $2, 'partner', 'admin')",
    )
    .bind(proj_id)
    .bind(org_id)
    .execute(pool)
    .await
    .unwrap();

    // Map 'member' -> 'admin'
    sqlx::query(
        "INSERT INTO project_org_role_access (project_id, organization_id, org_role, project_role)
         VALUES ($1, $2, 'member', 'admin')",
    )
    .bind(proj_id)
    .bind(org_id)
    .execute(pool)
    .await
    .unwrap();

    // Evan is in project_members as leader. Remove any direct grant so Evan's access is solely via Org
    sqlx::query("DELETE FROM project_member_grants WHERE project_id = $1 AND user_id = 5 AND grant_key = 'direct'")
        .bind(proj_id)
        .execute(pool)
        .await
        .unwrap();

    // Attempting to remove the Organization from the project while Evan is the leader -> MUST BE BLOCKED!
    let delete_org_access = sqlx::query("DELETE FROM project_organizations WHERE project_id = $1 AND organization_id = $2")
        .bind(proj_id)
        .bind(org_id)
        .execute(pool)
        .await;
    assert!(delete_org_access.is_err(), "Should prevent deleting organization when leader joined solely via org");

    // Attempting to remove Evan from the Organization while leader -> MUST BE BLOCKED!
    let delete_member = sqlx::query("DELETE FROM organization_members WHERE organization_id = $1 AND user_id = 5")
        .bind(org_id)
        .execute(pool)
        .await;
    assert!(delete_member.is_err(), "Should prevent removing user from org when leader of a cascaded project");

    // Handover leadership of Guild Project to Alice (user 1)
    sqlx::query("UPDATE project SET leader_id = 1 WHERE id = $1")
        .bind(proj_id)
        .execute(pool)
        .await
        .unwrap();

    // Now removing the Organization succeeds!
    let delete_org_access_ok = sqlx::query("DELETE FROM project_organizations WHERE project_id = $1 AND organization_id = $2")
        .bind(proj_id)
        .bind(org_id)
        .execute(pool)
        .await;
    assert!(delete_org_access_ok.is_ok(), "Disaffiliation should succeed once leadership is handed over");
}

#[tokio::test]
async fn test_active_tasks_preflight_warning_and_handover() {
    let pool = get_db().await;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();

    // Create Org
    let org_id: i64 = sqlx::query_scalar(
        "INSERT INTO organization (name, slug, governing_user_id) VALUES ($1, $2, 1) RETURNING id",
    )
    .bind("Contractor Firm")
    .bind(format!("contractor-firm-{}", now))
    .fetch_one(pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO organization_role (organization_id, role) VALUES ($1, 'engineering'), ($1, 'devops')")
        .bind(org_id)
        .execute(pool)
        .await
        .unwrap();

    // Add Evan (5) with role 'engineering'
    sqlx::query("INSERT INTO organization_members (organization_id, user_id, role) VALUES ($1, 5, 'engineering')")
        .bind(org_id)
        .execute(pool)
        .await
        .unwrap();

    // Create Project led by Alice (1)
    let proj_id: i64 = sqlx::query_scalar(
        "INSERT INTO project (name, description, leader_id) VALUES ($1, 'Client Portal', 1) RETURNING id",
    )
    .bind(format!("Client Portal - {}", now))
    .fetch_one(pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO project_members (project_id, user_id, role, permissions_mask) VALUES ($1, 1, 'admin', 63)")
        .bind(proj_id)
        .execute(pool)
        .await
        .unwrap();

    // Affiliate Org as contractor/member
    sqlx::query(
        "INSERT INTO project_organizations (project_id, organization_id, relationship_type, max_granted_role)
         VALUES ($1, $2, 'contractor', 'member')",
    )
    .bind(proj_id)
    .bind(org_id)
    .execute(pool)
    .await
    .unwrap();

    // Map 'engineering' -> 'member'
    sqlx::query(
        "INSERT INTO project_org_role_access (project_id, organization_id, org_role, project_role)
         VALUES ($1, $2, 'engineering', 'member')",
    )
    .bind(proj_id)
    .bind(org_id)
    .execute(pool)
    .await
    .unwrap();

    // Create an active task assigned to Evan (5)
    let task_id: i64 = sqlx::query_scalar(
        "INSERT INTO task (project_id, title, author, status) VALUES ($1, 'Build Auth Bridge', 1, 'assigned') RETURNING id",
    )
    .bind(proj_id)
    .fetch_one(pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO assignments (task_id, assigned_to) VALUES ($1, 5)")
        .bind(task_id)
        .execute(pool)
        .await
        .unwrap();

    // 1. Run the active task impact check: Should find 1 affected task
    let affected_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT t.id)
         FROM project_member_grants pmg
         JOIN project p ON p.id = pmg.project_id
         JOIN assignments a ON a.assigned_to = 5
         JOIN task t ON t.id = a.task_id AND t.project_id = p.id
         WHERE pmg.source_org_id = $1
           AND pmg.source_org_role = 'engineering'
           AND pmg.user_id = 5
           AND t.status != 'finished'
           AND NOT EXISTS (
               SELECT 1 FROM project_member_grants other
               JOIN project_role_definition prd ON prd.role = other.project_role
               WHERE other.project_id = p.id
                 AND other.user_id = 5
                 AND NOT (other.source_org_id = $1 AND other.source_org_role = 'engineering')
                 AND (prd.permissions_mask & 4) != 0
           )",
    )
    .bind(org_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(affected_count, 1, "Should identify Evan's active task on Client Portal");

    // 2. Multi-role potato person scenario: Grant Evan a second qualifying role ('devops' -> 'member')
    sqlx::query(
        "INSERT INTO project_org_role_access (project_id, organization_id, org_role, project_role)
         VALUES ($1, $2, 'devops', 'member')",
    )
    .bind(proj_id)
    .bind(org_id)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO organization_member_roles (organization_id, user_id, role) VALUES ($1, 5, 'devops')")
        .bind(org_id)
        .execute(pool)
        .await
        .unwrap();

    // Now re-running impact check when revoking 'engineering':
    // Since Evan still has 'devops' granting task editing permissions, affected count is 0 (NO false-positive warning!)
    let affected_count_after_second_role: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT t.id)
         FROM project_member_grants pmg
         JOIN project p ON p.id = pmg.project_id
         JOIN assignments a ON a.assigned_to = 5
         JOIN task t ON t.id = a.task_id AND t.project_id = p.id
         WHERE pmg.source_org_id = $1
           AND pmg.source_org_role = 'engineering'
           AND pmg.user_id = 5
           AND t.status != 'finished'
           AND NOT EXISTS (
               SELECT 1 FROM project_member_grants other
               JOIN project_role_definition prd ON prd.role = other.project_role
               WHERE other.project_id = p.id
                 AND other.user_id = 5
                 AND NOT (other.source_org_id = $1 AND other.source_org_role = 'engineering')
                 AND (prd.permissions_mask & 4) != 0
           )",
    )
    .bind(org_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(affected_count_after_second_role, 0, "No impact warning because Evan holds a second qualifying role");

    // 3. In-progress task guardrail: Set task to in_progress
    sqlx::query("UPDATE task SET status = 'in_progress' WHERE id = $1")
        .bind(task_id)
        .execute(pool)
        .await
        .unwrap();

    // Trigger prevent_unassign_in_progress_task protects Evan from being unassigned as the sole assignee of an in-progress task!
    let unassign_err = sqlx::query("DELETE FROM assignments WHERE task_id = $1 AND assigned_to = 5")
        .bind(task_id)
        .execute(pool)
        .await;
    assert!(unassign_err.is_err(), "Trigger should prevent unassigning the last assignee from an in-progress task");
}

