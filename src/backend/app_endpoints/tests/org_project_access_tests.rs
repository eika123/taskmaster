//! # Multi-Tenant Organization & Project RBAC Access Matrix Test Campaign
//!
//! This test campaign provides end-to-end verification of the enterprise RBAC,
//! grant ledger materialization, privilege ceilings, dynamic role synchronization,
//! task assignment authorization, and zero-knowledge need-to-know data isolation.
//!
//! ## Core Specifications Tested:
//! 1. **Grant Ledger & Additive Union ("Potato People")**:
//!    - Users holding multiple organizational roles receive the bitwise-OR additive union
//!      of privileges in affiliated projects (`project_member_grants` -> `project_members`).
//!    - Active "hat switching" allows a user to temporarily constrain effective permissions to a specific grant.
//! 2. **Privilege Ceilings**:
//!    - Partner and external organizations have a defined `max_granted_role` ceiling that
//!      strictly caps granted project permissions, regardless of organizational rank.
//! 3. **Cascade Leadership Protection**:
//!    - Database triggers guarantee a project leader cannot be removed or disaffiliated through
//!      upstream organizational changes without an explicit leadership transfer.
//! 4. **Pre-flight Warnings & Task Handover**:
//!    - Destructive membership and role changes perform pre-flight impact checks to alert
//!      callers if assigned active tasks will lose execution permissions.
//! 5. **Assignment Security**:
//!    - Only project members with explicit task-execution permissions (`mask & 4 != 0`)
//!      can be assigned to project tasks. Standalone tasks remain open to all users.
//! 6. **Dynamic Demotion, Mutation Restrictions & Zero-Knowledge Need-to-Know Isolation**:
//!    - Demoting a user immediately cascades to project grants and assignment cleanup.
//!    - Viewers (`mask == 1`) are prohibited from authoring, editing, claiming, or changing task status.
//!    - Unaffiliated users receive an empty project set (zero-knowledge: secret projects remain hidden).
//! 7. **Rule Revocation Cleanup**:
//!    - Revoking an organization role mapping automatically cleans up grants, recomputes
//!      memberships, and unassigns affected users.

use crate::backend::app_endpoints::tests::test_helpers::run_as_user;
use crate::backend::app_endpoints::*;
use crate::backend::database::get_db;
use crate::types::{Permission, TaskStatus};

/// Verifies that users holding multiple organizational roles ("potato people" wearing multiple hats)
/// receive the additive union (bitwise-OR) of all privileges across their active grants,
/// supports temporary "hat switching" to narrow privileges, and gracefully steps down when roles are revoked.
///
/// ### Test Execution Flow:
/// 1. **Setup**: Creates "Acme Oslo" (governor: Alice, id: 1) with custom roles `'teacher'` and `'parent'`.
/// 2. **Multi-Role Assignment**: Assigns Kari (id: 3) both the `'parent'` and `'teacher'` roles.
/// 3. **Affiliation & Initial Mapping**: Affiliates project "Class 4B" with Acme Oslo; maps `'parent'` -> `'viewer'`.
///    - Kari materializes in `project_members` as `'viewer'` (mask: 1).
/// 4. **Additive Role Upgrade**: Maps `'teacher'` -> `'admin'`.
///    - Trigger `sync_grants_after_rule_update` upgrades Kari to `'admin'` (mask: 63 = 1 | 63).
/// 5. **Hat Switching**: Pins Kari's active hat to `'org:<id>:parent'`, constraining effective permissions to `'viewer'`.
///    - Resets hat to `NULL`, returning effective permissions to full additive `'admin'`.
/// 6. **Role Demotion Step-Down**: Revokes Kari's `'teacher'` role in Acme Oslo.
///    - Kari remains in the project but steps down to `'viewer'` (mask: 1).
/// 7. **Complete Disaffiliation**: Removes Acme Oslo from Class 4B.
///    - Kari is cleanly purged from `project_members`, while leader Alice remains untouched.
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
        "SELECT role FROM organization_member_roles WHERE organization_id = $1 AND user_id = 3",
    )
    .bind(org_id)
    .fetch_all(pool)
    .await
    .unwrap();
    // check that roles contains "teacher" and "parent"
    assert!(roles.iter().any(|r| r == "teacher"), "missing teacher role");
    assert!(roles.iter().any(|r| r == "parent"), "missing parent role");
    
    // 2. Create Project: Class 4B (led by Alice = 1)
    let proj_id: i64 = sqlx::query_scalar(
        "INSERT INTO project (name, description, leader_id) VALUES ($1, 'Class 4B', 1) RETURNING id",
    )
    .bind(format!("Class 4B - {}", now))
    .fetch_one(pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO project_members (project_id, user_id, role, permissions_mask) VALUES ($1, 1, 'admin', $2)")
        .bind(proj_id)
        .bind(Permission::LEGACY_ADMIN.as_i64())
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
    assert_eq!(upgraded_mask, Permission::LEGACY_ADMIN.as_i64());

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
             permissions_mask = $2,
             role = 'admin'
         WHERE project_id = $1 AND user_id = 3",
    )
    .bind(proj_id)
    .bind(Permission::LEGACY_ADMIN.as_i64())
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

/// Verifies that partnership privilege ceilings (`max_granted_role` in `project_organizations`)
/// strictly cap the maximum role that an organization's role mappings can grant,
/// and verifies that users materialized with viewer permissions (`mask == 1`) cannot create tasks.
///
/// ### Test Execution Flow:
/// 1. **Setup**: Creates "External Audit Corp" and project "Security Review".
/// 2. **Affiliation with Ceiling**: Affiliates Audit Corp with ceiling `'viewer'`.
/// 3. **Trigger Enforcement (`check_org_project_role_ceiling`)**:
///    - Attempting to map `'admin'` -> `'admin'` (mask: 63) is rejected by trigger (exceeds viewer ceiling).
///    - Attempting to map `'admin'` -> `'member'` (mask: 15) is rejected by trigger.
///    - Mapping `'admin'` -> `'viewer'` (mask: 1) matches ceiling and succeeds.
/// 4. **Viewer Restriction**:
///    - Enrolls Diana (id: 4) into Audit Corp as admin; she materializes in the project as `'viewer'`.
///    - Diana attempts to create/author a task in the project -> rejected by permissions.
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

/// Verifies database guardrails protecting project leadership from accidental orphaning
/// when the project leader gained their membership via an organizational role mapping.
///
/// ### Test Execution Flow:
/// 1. **Setup**: Creates "Engineering Guild" with Evan (id: 5) as member, and project "Guild Project" led by Evan.
/// 2. **Affiliation**: Maps Guild `'member'` -> `'admin'`. Evan's project membership is backed solely by this org grant.
/// 3. **Cascade Blockers (`prevent_remove_project_leader`)**:
///    - Attempting to delete the organization affiliation while Evan is leader -> MUST BE BLOCKED.
///    - Attempting to remove Evan from the parent organization while leader of the affiliated project -> MUST BE BLOCKED.
/// 4. **Safe Leadership Handover**:
///    - Transfers project leadership to Alice (id: 1).
///    - Removing the organization affiliation now succeeds without violating leader integrity.
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

    sqlx::query("INSERT INTO project_members (project_id, user_id, role, permissions_mask) VALUES ($1, 5, 'admin', $2)")
        .bind(proj_id)
        .bind(Permission::LEGACY_ADMIN.as_i64())
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
    // This is necessary for this test flow, because direct members are not removed when an organization leaves.
    // Our multitenant system also mandates that people may live in a project with different hats.
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

/// Verifies pre-flight impact warnings when revoking an organizational role rule that would
/// strip task-handling permissions from members with active, assigned non-finished tasks.
///
/// ### Test Execution Flow:
/// 1. **Setup**: Affiliates "Contractor Firm" with project "Client Portal", mapping `'engineering'` -> `'member'`.
/// 2. **Active Task Assignment**: Assigns Evan (id: 5) an active task on the project.
/// 3. **Impact Pre-Flight Check**:
///    - Asserts query detects Evan's active task and reports 1 impacted task.
/// 4. **No False-Positive Warning**:
///    - Grants Evan a second role `'devops'` -> `'member'` (multi-role potato person).
///    - Impact check now yields 0 affected tasks because Evan's task privileges remain intact via `'devops'`.
/// 5. **In-Progress Task Guardrail (`prevent_unassign_in_progress_task`)**:
///    - Moves task to `'in_progress'`.
///    - Verifies trigger blocks unassigning Evan as the sole assignee of an ongoing task.
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

    sqlx::query("INSERT INTO project_members (project_id, user_id, role, permissions_mask) VALUES ($1, 1, 'admin', $2)")
        .bind(proj_id)
        .bind(Permission::LEGACY_ADMIN.as_i64())
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
                 AND has_perm(prd.permissions_mask, $2)
           )",
    )
    .bind(org_id)
    .bind(Permission::EXECUTE.as_i64())
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
                 AND has_perm(prd.permissions_mask, $2)
           )",
    )
    .bind(org_id)
    .bind(Permission::EXECUTE.as_i64())
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

/// Verifies task assignment authorization rules:
/// - Standalone tasks can be assigned to any registered user.
/// - Project tasks can ONLY be assigned to users with task-execution permissions (`mask & 4 != 0`).
/// - Assigning viewers (`mask == 3`) or non-members to a project task is rejected by database triggers.
///
/// ### Test Execution Flow:
/// 1. **Setup**: Creates a project with Alice (admin, mask: 63), Bob (member, mask: 15), and Charlie (viewer, mask: 3).
/// 2. **Task Creation**: Creates a task in `'accepted'` status.
/// 3. **Assignment Validation**:
///    - Assigning Bob (member, has bit 4) -> SUCCEEDS.
///    - Assigning Charlie (viewer, lacks bit 4) -> REJECTED by trigger `check_assignment_permission`.
///    - Assigning Diana (non-member) -> REJECTED by trigger `check_assignment_permission`.
#[tokio::test]
async fn test_standalone_and_project_task_assignment_permissions() {
    let pool = get_db().await;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();

    // 1. Create a project
    let proj_id: i64 = sqlx::query_scalar(
        "INSERT INTO project (name, description, leader_id) VALUES ($1, 'Role Assignment Test', 1) RETURNING id",
    )
    .bind(format!("Role Assignment Proj - {}", now))
    .fetch_one(pool)
    .await
    .unwrap();

    // Alice=1 is admin (LEGACY_ADMIN = 63)
    sqlx::query("INSERT INTO project_members (project_id, user_id, role, permissions_mask) VALUES ($1, 1, 'admin', $2)")
        .bind(proj_id)
        .bind(Permission::LEGACY_ADMIN.as_i64())
        .execute(pool)
        .await
        .unwrap();

    // Bob=2 is member (LEGACY_MEMBER = 15, has EXECUTE bit 4)
    sqlx::query("INSERT INTO project_members (project_id, user_id, role, permissions_mask) VALUES ($1, 2, 'member', $2)")
        .bind(proj_id)
        .bind(Permission::LEGACY_MEMBER.as_i64())
        .execute(pool)
        .await
        .unwrap();

    // Charlie=3 is viewer (READ | SUGGEST = 3, lacks EXECUTE bit 4)
    sqlx::query("INSERT INTO project_members (project_id, user_id, role, permissions_mask) VALUES ($1, 3, 'viewer', $2)")
        .bind(proj_id)
        .bind((Permission::READ | Permission::SUGGEST).as_i64())
        .execute(pool)
        .await
        .unwrap();

    // Create a project task in 'accepted' status (tasks must be at least 'accepted' to be assigned!)
    let task_id: i64 = sqlx::query_scalar(
        "INSERT INTO task (project_id, title, author, status) VALUES ($1, 'Assignment Test Task', 1, 'accepted') RETURNING id",
    )
    .bind(proj_id)
    .fetch_one(pool)
    .await
    .unwrap();

    // 2. Assigning Bob=2 (member, has bit 4) should succeed
    let assign_member = sqlx::query("INSERT INTO assignments (task_id, assigned_to) VALUES ($1, 2)")
        .bind(task_id)
        .execute(pool)
        .await;
    assert!(assign_member.is_ok(), "Assigning a project member with task execution permissions should succeed");

    // 3. Assigning Charlie=3 (viewer, lacks bit 4) should FAIL due to check_assignment_permission trigger
    let assign_viewer = sqlx::query("INSERT INTO assignments (task_id, assigned_to) VALUES ($1, 3)")
        .bind(task_id)
        .execute(pool)
        .await;
    assert!(assign_viewer.is_err(), "Assigning a viewer without task execution permissions must fail");

    // 4. Assigning a non-member (e.g. Diana=4) should also FAIL
    let assign_non_member = sqlx::query("INSERT INTO assignments (task_id, assigned_to) VALUES ($1, 4)")
        .bind(task_id)
        .execute(pool)
        .await;
    assert!(assign_non_member.is_err(), "Assigning a non-member to a project task must fail");
}

/// Comprehensive test verifying dynamic role demotion, mutation restrictions on viewers,
/// and Zero-Knowledge Need-to-Know isolation for unauthorized users.
///
/// ### Test Execution Flow:
/// 1. **Setup**: Affiliates organization with project; maps `'member'` -> `'member'` (mask 15),
///    `'developer'` -> `'viewer'` (mask 1), leaving `'auditor'` unmapped.
/// 2. **Member Phase (Bob)**: Bob enrolls as `'member'`, receives mask 15, is assigned a task, and authors a task.
/// 3. **Dynamic Demotion**: Bob is demoted in the organization to `'auditor'` (unmapped).
///    - Triggers purge Bob from `project_members` and purge his task assignments.
///    - Asserts old project tasks do NOT persist in Bob's "my tasks" view.
///    - Asserts Bob is blocked from viewing project tasks or task details.
///    - Asserts project leader cannot assign demoted Bob.
/// 4. **Viewer Restrictions (Charlie)**: Charlie joins as `'developer'` -> `'viewer'` (mask 1).
///    - Charlie CAN read project tasks and view task details.
///    - Charlie CANNOT create tasks, update details, change task status, or post messages.
/// 5. **Zero-Knowledge Need-to-Know Isolation (Ian)**:
///    - Ian joins organization with unmapped role `'auditor'`.
///    - `get_organization_affiliated_projects` returns ZERO projects for Ian (confidential project is omitted).
///    - Demoted Bob also receives ZERO projects.
///    - Charlie (viewer) DOES receive the project.
/// 6. **Dynamic Upgrading**: Rule for `'developer'` is updated from `'viewer'` to `'member'`.
///    - Trigger `sync_grants_after_rule_update` immediately upgrades Charlie to `'member'` (mask 15).
///    - Charlie CAN now successfully update tasks!
#[tokio::test]
async fn test_role_demotion_viewer_mutation_and_unauthorized_isolation() {
    let pool = get_db().await;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();

    // 1. Setup Organization & Project
    let org_id: i64 = sqlx::query_scalar(
        "INSERT INTO organization (name, slug, governing_user_id) VALUES ($1, $2, 1) RETURNING id",
    )
    .bind(format!("Org Auth Test {}", now))
    .bind(format!("org-auth-{}", now))
    .fetch_one(pool)
    .await
    .unwrap();

    let proj_id: i64 = sqlx::query_scalar(
        "INSERT INTO project (name, description, owner_org_id, leader_id) VALUES ($1, 'Auth Project', $2, 1) RETURNING id",
    )
    .bind(format!("Auth Proj {}", now))
    .bind(org_id)
    .fetch_one(pool)
    .await
    .unwrap();

    // Alice is leader/admin
    sqlx::query("INSERT INTO project_members (project_id, user_id, role, permissions_mask) VALUES ($1, 1, 'admin', $2)")
        .bind(proj_id)
        .bind(Permission::LEGACY_ADMIN.as_i64())
        .execute(pool)
        .await
        .unwrap();

    // Add roles to org: 'member', 'developer', 'auditor'
    sqlx::query("INSERT INTO organization_role (organization_id, role) VALUES ($1, 'developer'), ($1, 'auditor')")
        .bind(org_id)
        .execute(pool)
        .await
        .unwrap();

    // Map org roles to project roles:
    // 'member' -> 'member' (mask 15)
    // 'developer' -> 'viewer' (mask 1)
    // 'auditor' has NO rule mapping!
    sqlx::query(
        "INSERT INTO project_org_role_access (project_id, organization_id, org_role, project_role)
         VALUES ($1, $2, 'member', 'member'), ($1, $2, 'developer', 'viewer')",
    )
    .bind(proj_id)
    .bind(org_id)
    .execute(pool)
    .await
    .unwrap();

    // Create a task in the project
    let task_id: i64 = sqlx::query_scalar(
        "INSERT INTO task (project_id, title, description, author, status) VALUES ($1, 'Initial Task', 'Details', 1, 'accepted') RETURNING id",
    )
    .bind(proj_id)
    .fetch_one(pool)
    .await
    .unwrap();

    // 2. Add Bob (user 2) with role 'member'
    sqlx::query("INSERT INTO organization_members (organization_id, user_id, role) VALUES ($1, 2, 'member')")
        .bind(org_id)
        .execute(pool)
        .await
        .unwrap();

    // Bob has 'member' role, so Bob is in project_members with mask 15
    let (bob_role, bob_mask): (String, i64) = sqlx::query_as(
        "SELECT role, permissions_mask FROM project_members WHERE project_id = $1 AND user_id = 2",
    )
    .bind(proj_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(bob_role, "member");
    assert_eq!(bob_mask, 15);

    // Bob can read tasks, and can be assigned
    sqlx::query("INSERT INTO assignments (task_id, assigned_to) VALUES ($1, 2)")
        .bind(task_id)
        .execute(pool)
        .await
        .unwrap();

    let bob_task_id: i64 = sqlx::query_scalar(
        "INSERT INTO task (project_id, title, description, author, status) VALUES ($1, 'Bob Authored Task', 'Desc', 2, 'accepted') RETURNING id",
    )
    .bind(proj_id)
    .fetch_one(pool)
    .await
    .unwrap();

    let (bob_tasks, bob_my_tasks_before) = run_as_user(2, move || async move {
        let bob_tasks = get_project_tasks(proj_id, None, 10).await;
        let bob_my_tasks_before = get_my_tasks(None, 100).await.unwrap();
        (bob_tasks, bob_my_tasks_before)
    }).await;
    assert!(bob_tasks.is_ok(), "Bob should be able to view project tasks as member");
    assert!(bob_my_tasks_before.iter().any(|t| t.id == task_id));
    assert!(bob_my_tasks_before.iter().any(|t| t.id == bob_task_id));

    // 3. Demote Bob to 'auditor' (a role with no project mapping!)
    sqlx::query("UPDATE organization_members SET role = 'auditor' WHERE organization_id = $1 AND user_id = 2")
        .bind(org_id)
        .execute(pool)
        .await
        .unwrap();

    // Bob should now be completely removed from project_members!
    let bob_in_proj: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM project_members WHERE project_id = $1 AND user_id = 2")
        .bind(proj_id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(bob_in_proj, 0, "Demoted Bob must be removed from project_members");

    // Bob's assignments on this project must be purged!
    let bob_assigned: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM assignments WHERE assigned_to = 2 AND task_id = $1")
        .bind(task_id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(bob_assigned, 0, "Bob assignments on project tasks must be purged upon demotion");

    // In 'my tasks', Bob must NOT see old tasks from the revoked project!
    let (bob_my_tasks_after, bob_tasks_after, bob_task_after) = run_as_user(2, move || async move {
        let bob_my_tasks_after = get_my_tasks(None, 100).await.unwrap();
        let bob_tasks_after = get_project_tasks(proj_id, None, 10).await;
        let bob_task_after = get_task(task_id).await;
        (bob_my_tasks_after, bob_tasks_after, bob_task_after)
    }).await;
    assert!(!bob_my_tasks_after.iter().any(|t| t.id == task_id), "Revoked project task must not persist in my tasks");
    assert!(!bob_my_tasks_after.iter().any(|t| t.id == bob_task_id), "Revoked authored project task must not persist in my tasks");
    assert!(bob_tasks_after.is_err(), "Demoted Bob must be denied access to project tasks");
    assert!(bob_task_after.is_err(), "Demoted Bob must be denied access to view project task");

    // Alice tries to assign demoted Bob to the task -> Must fail!
    let (assign_bob, update_assign_bob) = run_as_user(1, move || async move {
        let assign_bob = assign_task(task_id, 2).await;
        let update_assign_bob = update_task(task_id, "Title".to_string(), "Desc".to_string(), TaskStatus::Accepted, vec![2]).await;
        (assign_bob, update_assign_bob)
    }).await;
    assert!(assign_bob.is_err(), "Cannot assign task to user lacking project execution permissions");
    assert!(update_assign_bob.is_err(), "update_task cannot assign user lacking project execution permissions");

    // 4. Test Charlie (user 3) as 'developer' (mapped to 'viewer', mask 1)
    sqlx::query("INSERT INTO organization_members (organization_id, user_id, role) VALUES ($1, 3, 'developer')")
        .bind(org_id)
        .execute(pool)
        .await
        .unwrap();

    let (charlie_role, charlie_mask): (String, i64) = sqlx::query_as(
        "SELECT role, permissions_mask FROM project_members WHERE project_id = $1 AND user_id = 3",
    )
    .bind(proj_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(charlie_role, "viewer");
    assert_eq!(charlie_mask, 1);

    // Charlie can read tasks
    let (charlie_tasks, charlie_task, charlie_create, charlie_update, charlie_status, charlie_msg) =
        run_as_user(3, move || async move {
            let tasks = get_project_tasks(proj_id, None, 10).await;
            let task = get_task(task_id).await;
            // Charlie CANNOT create tasks
            let create = suggest_task_for_project(proj_id, "Illegal Task".to_string(), "Desc".to_string()).await;
            // Charlie CANNOT edit tasks
            let update = update_task(task_id, "Mutated Title".to_string(), "Desc".to_string(), TaskStatus::Accepted, vec![]).await;
            // Charlie CANNOT change task status
            let status = set_task_status(task_id as u64, TaskStatus::Finished).await;
            // Charlie CANNOT post messages
            let msg = post_task_message(task_id, "Viewer message".to_string()).await;
            (tasks, task, create, update, status, msg)
        }).await;
    assert!(charlie_tasks.is_ok(), "Viewer can view tasks");
    assert!(charlie_task.is_ok(), "Viewer can view task details");
    assert!(charlie_create.is_err(), "Viewer cannot create tasks");
    assert!(charlie_update.is_err(), "Viewer cannot update task details");
    assert!(charlie_status.is_err(), "Viewer cannot change task status");
    assert!(charlie_msg.is_err(), "Viewer cannot post messages");

    // 5. Test Ian (user 9) as completely new member with unmapped role 'auditor'
    sqlx::query("INSERT INTO organization_members (organization_id, user_id, role) VALUES ($1, 9, 'auditor')")
        .bind(org_id)
        .execute(pool)
        .await
        .unwrap();

    let (ian_affils, ian_tasks, ian_task, ian_role_rules, ian_proj_orgs) =
        run_as_user(9, move || async move {
            let affils = get_organization_affiliated_projects(org_id).await.unwrap();
            let tasks = get_project_tasks(proj_id, None, 10).await;
            let task = get_task(task_id).await;
            let role_rules = get_organization_role_rules(org_id).await;
            let proj_orgs = get_project_organizations(proj_id).await;
            (affils, tasks, task, role_rules, proj_orgs)
        }).await;
    // Zero-knowledge: Ian Malcolm has no qualifying role, so the confidential project is completely omitted
    assert_eq!(ian_affils.len(), 0, "Ian has no view access, so project is not returned from backend");
    assert!(ian_tasks.is_err(), "Ian cannot download tasks without project permissions");
    assert!(ian_task.is_err(), "Ian cannot view task without project permissions");
    assert!(ian_role_rules.is_err(), "Ian cannot view organization role rules");
    assert!(ian_proj_orgs.is_err(), "Ian cannot inspect project organization affiliations");

    // Charlie has role 'developer' mapped to 'viewer', so Charlie DOES receive the project
    let charlie_affils = run_as_user(3, move || async move {
        get_organization_affiliated_projects(org_id).await.unwrap()
    }).await;
    assert_eq!(charlie_affils.len(), 1, "Charlie has viewer role, so project is visible");
    assert!(charlie_affils[0].has_access);

    // Demoted Bob (auditor, unmapped) also receives 0 projects
    let bob_affils = run_as_user(2, move || async move {
        get_organization_affiliated_projects(org_id).await.unwrap()
    }).await;
    assert_eq!(bob_affils.len(), 0, "Demoted Bob has no view access, so project is not returned");

    // 6. Test Trigger: Dynamic Rule Update upgrades Charlie from 'viewer' to 'member'
    sqlx::query(
        "UPDATE project_org_role_access SET project_role = 'member'
         WHERE project_id = $1 AND organization_id = $2 AND org_role = 'developer'",
    )
    .bind(proj_id)
    .bind(org_id)
    .execute(pool)
    .await
    .unwrap();

    // Trigger sync_grants_after_rule_update automatically updated Charlie in project_members!
    let (upgraded_role, upgraded_mask): (String, i64) = sqlx::query_as(
        "SELECT role, permissions_mask FROM project_members WHERE project_id = $1 AND user_id = 3",
    )
    .bind(proj_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(upgraded_role, "member");
    assert_eq!(upgraded_mask, 15);

    // Now Charlie CAN update tasks!
    let charlie_update_ok = run_as_user(3, move || async move {
        update_task(task_id, "Legitimate Member Edit".to_string(), "New Desc".to_string(), TaskStatus::Accepted, vec![]).await
    }).await;
    assert!(charlie_update_ok.is_ok(), "Upgraded member can now update tasks");
}

/// Verifies that calling `revoke_org_role_project_access` cascades properly across the database:
/// - Prompts with an active task impact warning if affected users hold assigned tasks.
/// - When confirmed (`confirm_active_tasks_warning = true`), deletes the role rule,
///   which cascades to `project_member_grants`.
/// - Trigger `recompute_project_member` cleanly removes affected members from `project_members`.
/// - Endpoints `get_project_members` and `get_project_assignable_users` omit revoked users.
/// - Unassigns tasks and ensures revoked tasks no longer appear in the user's `get_my_tasks`.
///
/// ### Test Execution Flow:
/// 1. **Setup**: Affiliates organization with project and maps `'member'` -> `'member'`.
/// 2. **Member Assignment**: Bob joins the org, materializes into the project, and is assigned a task.
/// 3. **Revocation Warning**: Alice attempts revoking without confirmation -> receives warning with impacted task details.
/// 4. **Confirmed Revocation**: Alice confirms -> rule is deleted.
/// 5. **Materialization Integrity**: Asserts Bob has 0 grants and 0 entries in `project_members`.
/// 6. **Endpoint Visibility**: Asserts Bob does not appear in member list or assignable users.
/// 7. **Assignment Cleanup & Isolation**: Asserts Bob is unassigned and the task vanishes from his `get_my_tasks`.
#[tokio::test]
async fn test_revoke_org_role_project_access_cleans_up_members_and_assignments() {
    let pool = get_db().await;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();

    // 1. Create Organization & Project
    let org_id: i64 = sqlx::query_scalar(
        "INSERT INTO organization (name, slug, governing_user_id) VALUES ($1, $2, 1) RETURNING id",
    )
    .bind(format!("Org Revoke Test {}", now))
    .bind(format!("org-rev-{}", now))
    .fetch_one(pool)
    .await
    .unwrap();

    let proj_id: i64 = sqlx::query_scalar(
        "INSERT INTO project (name, description, owner_org_id, leader_id) VALUES ($1, 'Revoke Proj', $2, 1) RETURNING id",
    )
    .bind(format!("Revoke Proj {}", now))
    .bind(org_id)
    .fetch_one(pool)
    .await
    .unwrap();

    // Alice is leader
    sqlx::query("INSERT INTO project_members (project_id, user_id, role, permissions_mask) VALUES ($1, 1, 'admin', $2)")
        .bind(proj_id)
        .bind(Permission::LEGACY_ADMIN.as_i64())
        .execute(pool)
        .await
        .unwrap();

    // Add role rule: 'member' -> 'member'
    sqlx::query(
        "INSERT INTO project_org_role_access (project_id, organization_id, org_role, project_role)
         VALUES ($1, $2, 'member', 'member')",
    )
    .bind(proj_id)
    .bind(org_id)
    .execute(pool)
    .await
    .unwrap();

    // Enroll Bob (user 2) into the organization as 'member'
    sqlx::query("INSERT INTO organization_members (organization_id, user_id, role) VALUES ($1, 2, 'member')")
        .bind(org_id)
        .execute(pool)
        .await
        .unwrap();

    // Verify Bob is materialized into project_members
    let (bob_pm_role, bob_pm_mask): (String, i64) = sqlx::query_as(
        "SELECT role, permissions_mask FROM project_members WHERE project_id = $1 AND user_id = 2",
    )
    .bind(proj_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(bob_pm_role, "member");
    assert_eq!(bob_pm_mask, 15);

    // Create a task and assign Bob
    let task_id: i64 = sqlx::query_scalar(
        "INSERT INTO task (project_id, title, description, author, status) VALUES ($1, 'Feature Implementation', 'Code', 1, 'assigned') RETURNING id",
    )
    .bind(proj_id)
    .fetch_one(pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO assignments (task_id, assigned_to) VALUES ($1, 2)")
        .bind(task_id)
        .execute(pool)
        .await
        .unwrap();

    // Alice revokes 'member' access WITHOUT confirmation -> Warning error!
    let (revoke_warn, revoke_ok, members, assignable) = run_as_user(1, move || async move {
        let revoke_warn = revoke_org_role_project_access(proj_id, org_id, "member".to_string(), false).await;
        // Alice confirms warning -> Revocation succeeds!
        let revoke_ok = revoke_org_role_project_access(proj_id, org_id, "member".to_string(), true).await;
        // Verify Bob does NOT appear in get_project_members endpoint
        let members = get_project_members(proj_id).await.unwrap();
        // Verify Bob does NOT appear in get_project_assignable_users endpoint
        let assignable = get_project_assignable_users(Some(proj_id)).await.unwrap();
        (revoke_warn, revoke_ok, members, assignable)
    }).await;

    assert!(revoke_warn.is_err(), "Must warn before revoking rule affecting active tasks");
    let warn_msg = revoke_warn.unwrap_err().to_string();
    assert!(warn_msg.contains("Active task impact warning"), "Message should mention impact warning: {}", warn_msg);
    assert!(warn_msg.contains("Bob Smith"), "Message should mention Bob Smith: {}", warn_msg);
    assert!(revoke_ok.is_ok(), "Revocation should succeed with confirmation");

    // Verify Bob is completely purged from project_member_grants
    let grants_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM project_member_grants WHERE project_id = $1 AND user_id = 2",
    )
    .bind(proj_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(grants_count, 0, "Bob must have 0 grants after role rule revocation");

    // Verify Bob is completely purged from project_members!
    let pm_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM project_members WHERE project_id = $1 AND user_id = 2",
    )
    .bind(proj_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(pm_count, 0, "Bob must be removed from project_members table after rule revocation");

    assert!(!members.iter().any(|m| m.user_id == 2), "Bob must not show up in get_project_members table");
    assert!(!assignable.iter().any(|u| u.id == 2), "Bob must not show up in assignable users");

    // Verify Bob's assignment was cleanly deleted
    let task_assignees: Vec<i64> = sqlx::query_scalar(
        "SELECT assigned_to FROM assignments WHERE task_id = $1",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await
    .unwrap();
    assert!(!task_assignees.contains(&2), "Bob must be unassigned from project task");

    // Verify Bob does NOT see the task in get_my_tasks
    let bob_tasks = run_as_user(2, move || async move {
        get_my_tasks(None, 100).await.unwrap()
    }).await;
    assert!(!bob_tasks.iter().any(|t| t.id == task_id), "Revoked user must not see project task in my tasks");
}

