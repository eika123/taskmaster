use crate::backend::app_endpoints::*;
use crate::backend::database::get_db;

#[tokio::test]
async fn test_project_task_association_and_deletion_cleanup() {
    let pool = get_db().await;

    // 1. Create a project
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

    // 2. Insert tasks belonging to this project
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

    // 3. Verify get_task and get_project_tasks return the correct project_name
    let fetched_task1 = get_task(task1_id).await.unwrap();
    assert_eq!(fetched_task1.project_id, proj_id);
    assert_eq!(fetched_task1.project_name, "Project With Tasks");

    let project_tasks = get_project_tasks(proj_id, None, 10).await.unwrap();
    assert_eq!(project_tasks.len(), 2);
    assert_eq!(project_tasks[0].project_name, "Project With Tasks");
    assert_eq!(project_tasks[1].project_name, "Project With Tasks");

    // 4. Delete the project directly; DB triggers & foreign key CASCADE handle tasks
    sqlx::query("DELETE FROM project WHERE id = $1")
        .bind(proj_id)
        .execute(pool)
        .await
        .unwrap();

    // 5. Verify the tasks are completely deleted
    let count_tasks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM task WHERE project_id = $1")
        .bind(proj_id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(count_tasks, 0, "All tasks of deleted project should be removed");

    assert!(get_task(task1_id).await.is_err());
    assert!(get_task(task2_id).await.is_err());
}

