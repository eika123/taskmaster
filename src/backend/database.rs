#[cfg(feature = "server")]
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteJournalMode, SqliteSynchronous};
#[cfg(feature = "server")]
use std::str::FromStr;
#[cfg(feature = "server")]
use crate::backend::crypto::hash_password;

#[cfg(feature = "server")]
static POOL: tokio::sync::OnceCell<sqlx::SqlitePool> = tokio::sync::OnceCell::const_new();

#[cfg(feature = "server")]
pub async fn get_db() -> &'static sqlx::SqlitePool {
    POOL.get_or_init(|| async {
        let _ = std::fs::create_dir_all("data");
        let options = SqliteConnectOptions::from_str("sqlite:data/todolist.db")
            .expect("Invalid SQLite connection string")
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(std::time::Duration::from_millis(5000));

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .expect("Failed to connect to SQLite database");

        init_db(&pool).await.expect("Failed to initialize database schema");
        pool
    })
    .await
}

#[cfg(feature = "server")]
async fn init_db(pool: &sqlx::SqlitePool) -> Result<(), sqlx::Error> {
    // 1. Create base tables
    let tables_sql = "
        -- 1. Users & Sessions
        CREATE TABLE IF NOT EXISTS user (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL DEFAULT ''
        );

        CREATE TABLE IF NOT EXISTS session (
            token TEXT PRIMARY KEY,
            user_id INTEGER NOT NULL,
            created_at INTEGER NOT NULL,
            expires_at INTEGER NOT NULL,
            FOREIGN KEY(user_id) REFERENCES user(id) ON DELETE CASCADE
        );

        -- 2. Organizations & Extensible Organization Roles
        CREATE TABLE IF NOT EXISTS organization (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            slug TEXT NOT NULL UNIQUE,
            governing_user_id INTEGER NOT NULL REFERENCES user(id) ON DELETE RESTRICT,
            created_at INTEGER NOT NULL DEFAULT (unixepoch())
        );

        CREATE TABLE IF NOT EXISTS organization_role (
            organization_id INTEGER NOT NULL REFERENCES organization(id) ON DELETE CASCADE,
            role TEXT NOT NULL,
            PRIMARY KEY (organization_id, role)
        ) WITHOUT ROWID;

        CREATE TABLE IF NOT EXISTS organization_members (
            organization_id INTEGER NOT NULL REFERENCES organization(id) ON DELETE CASCADE,
            user_id INTEGER NOT NULL REFERENCES user(id) ON DELETE CASCADE,
            role TEXT NOT NULL DEFAULT 'member',
            joined_at INTEGER NOT NULL DEFAULT (unixepoch()),
            PRIMARY KEY (organization_id, user_id),
            FOREIGN KEY (organization_id, role)
                REFERENCES organization_role(organization_id, role)
                ON UPDATE CASCADE ON DELETE RESTRICT
        ) WITHOUT ROWID;

        CREATE INDEX IF NOT EXISTS idx_org_members_user ON organization_members(user_id, organization_id);
        CREATE INDEX IF NOT EXISTS idx_org_members_role ON organization_members(organization_id, role);

        CREATE TABLE IF NOT EXISTS organization_governance_transfer (
            organization_id INTEGER PRIMARY KEY REFERENCES organization(id) ON DELETE CASCADE,
            requested_new_governor INTEGER NOT NULL,
            created_at INTEGER NOT NULL DEFAULT (unixepoch()),
            FOREIGN KEY (organization_id, requested_new_governor)
                REFERENCES organization_members(organization_id, user_id) ON DELETE CASCADE
        ) WITHOUT ROWID;

        -- 3. Projects & Role-Based Access Policy
        CREATE TABLE IF NOT EXISTS project (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            owner_org_id INTEGER REFERENCES organization(id) ON DELETE RESTRICT,
            leader_id INTEGER NOT NULL REFERENCES user(id) ON DELETE RESTRICT,
            created_at INTEGER NOT NULL DEFAULT (unixepoch())
        );

        CREATE INDEX IF NOT EXISTS idx_project_owner_org ON project(owner_org_id) WHERE owner_org_id IS NOT NULL;
        CREATE INDEX IF NOT EXISTS idx_project_leader ON project(leader_id);

        CREATE TABLE IF NOT EXISTS project_members (
            project_id INTEGER NOT NULL REFERENCES project(id) ON DELETE CASCADE,
            user_id INTEGER NOT NULL REFERENCES user(id) ON DELETE CASCADE,
            role TEXT NOT NULL DEFAULT 'member' CHECK (role IN ('admin', 'member')),
            granted_by_org_role INTEGER NOT NULL DEFAULT 0,
            joined_at INTEGER NOT NULL DEFAULT (unixepoch()),
            PRIMARY KEY (project_id, user_id)
        ) WITHOUT ROWID;

        CREATE INDEX IF NOT EXISTS idx_proj_members_user ON project_members(user_id, project_id);

        CREATE TABLE IF NOT EXISTS project_org_role_access (
            project_id INTEGER NOT NULL REFERENCES project(id) ON DELETE CASCADE,
            organization_id INTEGER NOT NULL,
            role TEXT NOT NULL,
            PRIMARY KEY (project_id, organization_id, role),
            FOREIGN KEY (organization_id, role)
                REFERENCES organization_role(organization_id, role)
                ON UPDATE CASCADE ON DELETE CASCADE
        ) WITHOUT ROWID;

        CREATE TABLE IF NOT EXISTS project_leadership_transfer (
            project_id INTEGER PRIMARY KEY REFERENCES project(id) ON DELETE CASCADE,
            requested_new_leader INTEGER NOT NULL,
            created_at INTEGER NOT NULL DEFAULT (unixepoch()),
            FOREIGN KEY (project_id, requested_new_leader)
                REFERENCES project_members(project_id, user_id) ON DELETE CASCADE
        ) WITHOUT ROWID;

        -- 4. In-System Invitations & User Message Box
        CREATE TABLE IF NOT EXISTS project_invitation (
            project_id INTEGER NOT NULL REFERENCES project(id) ON DELETE CASCADE,
            invited_user_id INTEGER NOT NULL REFERENCES user(id) ON DELETE CASCADE,
            invited_by INTEGER REFERENCES user(id) ON DELETE SET NULL,
            role TEXT NOT NULL DEFAULT 'member' CHECK (role IN ('admin', 'member')),
            message TEXT NOT NULL DEFAULT '',
            created_at INTEGER NOT NULL DEFAULT (unixepoch()),
            PRIMARY KEY (project_id, invited_user_id)
        ) WITHOUT ROWID;

        CREATE INDEX IF NOT EXISTS idx_proj_invite_user ON project_invitation(invited_user_id);

        CREATE TABLE IF NOT EXISTS organization_invitation (
            organization_id INTEGER NOT NULL REFERENCES organization(id) ON DELETE CASCADE,
            invited_user_id INTEGER NOT NULL REFERENCES user(id) ON DELETE CASCADE,
            invited_by INTEGER REFERENCES user(id) ON DELETE SET NULL,
            role TEXT NOT NULL DEFAULT 'member',
            message TEXT NOT NULL DEFAULT '',
            created_at INTEGER NOT NULL DEFAULT (unixepoch()),
            PRIMARY KEY (organization_id, invited_user_id),
            FOREIGN KEY (organization_id, role)
                REFERENCES organization_role(organization_id, role)
                ON UPDATE CASCADE ON DELETE CASCADE
        ) WITHOUT ROWID;

        CREATE INDEX IF NOT EXISTS idx_org_invite_user ON organization_invitation(invited_user_id);

        CREATE TABLE IF NOT EXISTS user_inbox_message (
            id INTEGER PRIMARY KEY,
            recipient_user_id INTEGER NOT NULL REFERENCES user(id) ON DELETE CASCADE,
            sender_user_id INTEGER REFERENCES user(id) ON DELETE SET NULL,
            subject TEXT NOT NULL,
            body TEXT NOT NULL,
            is_read INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL DEFAULT (unixepoch())
        );

        CREATE INDEX IF NOT EXISTS idx_inbox_recipient ON user_inbox_message(recipient_user_id, is_read, created_at DESC);

        -- 5. Tasks, Assignments & Discussion Threads
        CREATE TABLE IF NOT EXISTS task (
            id INTEGER PRIMARY KEY,
            project_id INTEGER NOT NULL REFERENCES project(id) ON DELETE CASCADE,
            title TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            author INTEGER REFERENCES user(id) ON DELETE SET NULL,
            status TEXT NOT NULL DEFAULT 'suggested'
                CHECK (status IN ('suggested', 'accepted', 'assigned', 'in_progress', 'finished')),
            loro_snapshot BLOB
        );

        CREATE INDEX IF NOT EXISTS idx_task_project ON task(project_id, id);

        CREATE TABLE IF NOT EXISTS assignments (
            task_id INTEGER NOT NULL REFERENCES task(id) ON DELETE CASCADE,
            assigned_to INTEGER NOT NULL REFERENCES user(id) ON DELETE CASCADE,
            PRIMARY KEY (task_id, assigned_to)
        ) WITHOUT ROWID;

        CREATE INDEX IF NOT EXISTS idx_assignments_user ON assignments(assigned_to, task_id);

        CREATE TABLE IF NOT EXISTS task_message (
            task_id INTEGER NOT NULL REFERENCES task(id) ON DELETE CASCADE,
            msg_id INTEGER NOT NULL,
            author_id INTEGER REFERENCES user(id) ON DELETE SET NULL,
            created_at INTEGER NOT NULL DEFAULT (unixepoch()),
            msg TEXT NOT NULL,
            PRIMARY KEY (task_id, msg_id)
        ) WITHOUT ROWID;
    ";
    sqlx::raw_sql(tables_sql).execute(pool).await?;

    // 2. Triggers
    let triggers_sql = "
        -- Organization Role Trigger: Seed 'member' and 'admin' roles when an org is created
        CREATE TRIGGER IF NOT EXISTS seed_default_org_roles
        AFTER INSERT ON organization
        FOR EACH ROW
        BEGIN
            INSERT INTO organization_role (organization_id, role) VALUES
                (NEW.id, 'member'),
                (NEW.id, 'admin');
        END;

        -- Organization Role Trigger: Protect 'member' and 'admin' from deletion while org exists
        CREATE TRIGGER IF NOT EXISTS protect_builtin_org_roles
        BEFORE DELETE ON organization_role
        FOR EACH ROW
        WHEN OLD.role IN ('member', 'admin')
          AND EXISTS (SELECT 1 FROM organization WHERE id = OLD.organization_id)
        BEGIN
            SELECT RAISE(FAIL, 'Cannot delete built-in organization roles (admin, member).');
        END;

        -- Organization Guardrail: Cannot delete an organization while other members remain
        CREATE TRIGGER IF NOT EXISTS prevent_delete_populated_org
        BEFORE DELETE ON organization
        FOR EACH ROW
        WHEN EXISTS (
            SELECT 1 FROM organization_members
            WHERE organization_id = OLD.id AND user_id != OLD.governing_user_id
        )
        BEGIN
            SELECT RAISE(FAIL, 'Cannot delete an organization while other members remain. Remove all other members first.');
        END;

        -- Organization Guardrail: Cannot remove the governing user from organization_members
        CREATE TRIGGER IF NOT EXISTS prevent_remove_governing_user
        BEFORE DELETE ON organization_members
        FOR EACH ROW
        WHEN (SELECT governing_user_id FROM organization WHERE id = OLD.organization_id) = OLD.user_id
        BEGIN
            SELECT RAISE(FAIL, 'The governing user cannot be removed from the organization. Transfer governance first.');
        END;

        -- Project Guardrail: Cannot delete a project while other members remain
        CREATE TRIGGER IF NOT EXISTS prevent_delete_populated_project
        BEFORE DELETE ON project
        FOR EACH ROW
        WHEN EXISTS (
            SELECT 1 FROM project_members
            WHERE project_id = OLD.id AND user_id != OLD.leader_id
        )
        BEGIN
            SELECT RAISE(FAIL, 'Cannot delete a project while other members remain. Remove all other members first.');
        END;

        -- Project Guardrail: Cannot remove the project leader from project_members
        CREATE TRIGGER IF NOT EXISTS prevent_remove_project_leader
        BEFORE DELETE ON project_members
        FOR EACH ROW
        WHEN (SELECT leader_id FROM project WHERE id = OLD.project_id) = OLD.user_id
        BEGIN
            SELECT RAISE(FAIL, 'The project leader cannot be removed from the project. Transfer project leadership first.');
        END;

        -- Task Trigger 1: Cannot assign a task that is in 'suggested' status
        CREATE TRIGGER IF NOT EXISTS prevent_assign_suggested_task
        BEFORE INSERT ON assignments
        FOR EACH ROW
        WHEN (SELECT status FROM task WHERE id = NEW.task_id) = 'suggested'
        BEGIN
            SELECT RAISE(FAIL, 'Cannot assign a task that has suggested status');
        END;

        -- Task Trigger 2: Cannot change task status to 'in_progress' without at least one assignee
        CREATE TRIGGER IF NOT EXISTS enforce_in_progress_assigned
        BEFORE UPDATE OF status ON task
        FOR EACH ROW
        WHEN NEW.status = 'in_progress' AND NOT EXISTS (SELECT 1 FROM assignments WHERE task_id = NEW.id)
        BEGIN
            SELECT RAISE(FAIL, 'Cannot set status to in_progress without an assignee');
        END;

        -- Task Trigger 3: Cannot remove the last assignee if task is 'in_progress'
        CREATE TRIGGER IF NOT EXISTS prevent_unassign_in_progress_task
        BEFORE DELETE ON assignments
        FOR EACH ROW
        WHEN (SELECT status FROM task WHERE id = OLD.task_id) = 'in_progress' 
             AND (SELECT COUNT(*) FROM assignments WHERE task_id = OLD.task_id) <= 1
        BEGIN
            SELECT RAISE(FAIL, 'Cannot remove the last assignee from an in_progress task');
        END;
    ";
    sqlx::raw_sql(triggers_sql).execute(pool).await?;

    // 3. Seed users
    let seed_users_sql = "
        INSERT OR IGNORE INTO user (id, name, email, password_hash) VALUES
            (1, 'Alice Johnson', 'alice@example.com', ''),
            (2, 'Bob Smith', 'bob@example.com', ''),
            (3, 'Charlie Brown', 'charlie@example.com', ''),
            (4, 'Diana Prince', 'diana@example.com', ''),
            (5, 'Evan Wright', 'evan@example.com', ''),
            (6, 'Fiona Gallagher', 'fiona@example.com', ''),
            (7, 'George Clark', 'george@example.com', ''),
            (8, 'Hannah Abbott', 'hannah@example.com', ''),
            (9, 'Ian Malcolm', 'ian@example.com', ''),
            (10, 'Julia Roberts', 'julia@example.com', ''),
            (11, 'Fiona Hill', 'fiona-hill@example.com', ''),
            (12, 'Hannah Montana', 'hannah.montana@disney.com', ''),
            (13, 'Ewan McGregor', 'ewan.mcgregor@coruscant.com', ''),
            (14, 'Gordon Brown', 'gordon-brown@gov.uk', ''),
            (15, 'George Best', 'george.best@manchester.united.com', '');
    ";
    sqlx::raw_sql(seed_users_sql).execute(pool).await?;

    // 4. Seed default password ('password123')
    let default_hash = hash_password("password123").unwrap_or_default();
    if !default_hash.is_empty() {
        let _ = sqlx::query("UPDATE user SET password_hash = $1 WHERE password_hash = ''")
            .bind(&default_hash)
            .execute(pool)
            .await;
    }

    // 5. Seed default organization, projects, and members
    let seed_org_proj_sql = "
        -- Default organization: Acme Corp (governing user: Alice)
        INSERT OR IGNORE INTO organization (id, name, slug, governing_user_id)
        VALUES (1, 'Acme Corp', 'acme-corp', 1);

        -- Ensure Alice is in Acme Corp as admin
        INSERT OR IGNORE INTO organization_members (organization_id, user_id, role)
        VALUES (1, 1, 'admin');

        -- Add Bob to Acme Corp as member
        INSERT OR IGNORE INTO organization_members (organization_id, user_id, role)
        VALUES (1, 2, 'member');

        -- Default Project 1: Acme Engineering (Org-owned by Acme Corp, leader: Alice)
        INSERT OR IGNORE INTO project (id, name, description, owner_org_id, leader_id)
        VALUES (1, 'Acme Engineering', 'Core product development and backend services.', 1, 1);

        -- Project 1 members: Alice (admin), Bob (member)
        INSERT OR IGNORE INTO project_members (project_id, user_id, role)
        VALUES (1, 1, 'admin'), (1, 2, 'member');

        -- Default Project 2: Bob''s Personal Project (Standalone, leader: Bob)
        INSERT OR IGNORE INTO project (id, name, description, owner_org_id, leader_id)
        VALUES (2, 'Personal Roadmap', 'Standalone project roadmap and personal tasks.', NULL, 2);

        -- Project 2 members: Bob (admin)
        INSERT OR IGNORE INTO project_members (project_id, user_id, role)
        VALUES (2, 2, 'admin');

        -- Seed initial tasks for Project 1
        INSERT OR IGNORE INTO task (id, project_id, title, description, author, status) VALUES
            (1, 1, 'Refactor Authentication Pipeline', 'Migrate auth guards to Axum session pool and role checks.', 1, 'assigned'),
            (2, 1, 'Implement Loro CRDT Collaboration', 'Design ephemeral collaboration worker for real-time document editing.', 1, 'suggested'),
            (3, 1, 'Setup Automated CI/CD', 'Deploy staging pipeline with SQLite WAL and automated smoke tests.', 2, 'accepted');

        -- Assignments for Task 1
        INSERT OR IGNORE INTO assignments (task_id, assigned_to) VALUES
            (1, 1),
            (1, 2);

        -- Seed initial discussion message for Task 1
        INSERT OR IGNORE INTO task_message (task_id, msg_id, author_id, created_at, msg) VALUES
            (1, 1, 1, unixepoch() - 3600, 'I started moving the endpoints into backend/app_endpoints.rs.'),
            (1, 2, 2, unixepoch() - 1800, 'Looks great! I verified the session pool handles cookies correctly.');
    ";
    sqlx::raw_sql(seed_org_proj_sql).execute(pool).await?;

    // 6. Spawn background reconciliation for role-based project access
    spawn_role_access_reconciliation(pool.clone());

    Ok(())
}

#[cfg(feature = "server")]
fn spawn_role_access_reconciliation(pool: sqlx::SqlitePool) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
        loop {
            interval.tick().await;
            let _ = sqlx::query(
                "INSERT OR IGNORE INTO project_members (project_id, user_id, role, granted_by_org_role)
                 SELECT pora.project_id, om.user_id, 'member', 1
                 FROM project_org_role_access pora
                 INNER JOIN organization_members om
                     ON om.organization_id = pora.organization_id
                    AND om.role = pora.role"
            )
            .execute(&pool)
            .await;
        }
    });
}