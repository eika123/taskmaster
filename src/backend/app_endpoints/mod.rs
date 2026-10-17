mod auth;
mod tasks;
mod projects;
mod organizations;
mod dashboard;

pub use auth::*;
pub use tasks::*;
pub use projects::*;
pub use organizations::*;
pub use dashboard::*;


#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;
    use crate::backend::database::get_db;
    use crate::types::TaskStatus;

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
    async fn test_project_leadership_and_task_messages() {
        let pool = get_db().await;

        // 1. Create a project led by Bob
        let proj_id: i64 = sqlx::query_scalar(
            "INSERT INTO project (name, description, leader_id) VALUES ($1, $2, $3) RETURNING id",
        )
        .bind("Test Discussion Project")
        .bind("Testing discussion threads and clustered keys")
        .bind(2) // Bob
        .fetch_one(pool)
        .await
        .unwrap();

        sqlx::query("INSERT INTO project_members (project_id, user_id, role) VALUES ($1, $2, 'admin')")
            .bind(proj_id)
            .bind(2)
            .execute(pool)
            .await
            .unwrap();

        // 2. Add task
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

        // 3. Insert discussion messages directly to verify clustered sequence
        sqlx::query("INSERT INTO task_message (task_id, msg_id, author_id, created_at, msg) VALUES ($1, 1, 2, unixepoch(), 'Message 1')")
            .bind(task_id)
            .execute(pool)
            .await
            .unwrap();

        sqlx::query("INSERT INTO task_message (task_id, msg_id, author_id, created_at, msg) VALUES ($1, 2, 1, unixepoch(), 'Message 2')")
            .bind(task_id)
            .execute(pool)
            .await
            .unwrap();

        let messages = get_task_messages(task_id).await.unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].msg_id, 1);
        assert_eq!(messages[0].msg, "Message 1");
        assert_eq!(messages[1].msg_id, 2);
        assert_eq!(messages[1].msg, "Message 2");

        // 4. Test update_task with status triggers
        // Setting 'in_progress' without assignees should fail
        let update_fail = update_task(task_id, "Clustered Key Task".to_string(), "Desc".to_string(), TaskStatus::InProgress, vec![]).await;
        assert!(update_fail.is_err());

        // Assign user and set in_progress
        let update_ok = update_task(task_id, "Clustered Key Task Updated".to_string(), "New Desc".to_string(), TaskStatus::InProgress, vec![2]).await.unwrap();
        assert_eq!(update_ok.status, TaskStatus::InProgress);
        assert_eq!(update_ok.assignees.len(), 1);
        assert_eq!(update_ok.assignees[0].id, 2);
    }

    #[tokio::test]
    async fn test_project_invitation_flow() {
        let pool = get_db().await;

        let proj_id: i64 = sqlx::query_scalar(
            "INSERT INTO project (name, description, leader_id) VALUES ($1, $2, $3) RETURNING id",
        )
        .bind("Invitation Test Project")
        .bind("Verifying invite workflow")
        .bind(1) // Alice
        .fetch_one(pool)
        .await
        .unwrap();

        // 1. Insert invite for Bob (user 2)
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

        // Verify invite exists
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM project_invitation WHERE project_id = $1 AND invited_user_id = 2)",
        )
        .bind(proj_id)
        .fetch_one(pool)
        .await
        .unwrap();
        assert!(exists);

        // Accept invite: insert into project_members and delete from project_invitation
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

        // Check Bob is in project_members
        let is_member: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM project_members WHERE project_id = $1 AND user_id = 2)",
        )
        .bind(proj_id)
        .fetch_one(pool)
        .await
        .unwrap();
        assert!(is_member);
    }
}
