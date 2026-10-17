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
    // 0. Idempotent schema migrations for pre-existing database files
    let has_pm_table: bool = sqlx::query_scalar(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='project_members'"
    )
    .fetch_one(pool)
    .await
    .unwrap_or(false);

    if has_pm_table {
        let pm_sql: Option<String> = sqlx::query_scalar(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='project_members'"
        )
        .fetch_optional(pool)
        .await
        .unwrap_or(None);

        if let Some(sql) = pm_sql {
            if sql.contains("CHECK (role IN ('admin', 'member'))") {
                let migration_sql = "
                    PRAGMA foreign_keys = OFF;
                    DROP TRIGGER IF EXISTS prevent_delete_populated_project;
                    DROP TRIGGER IF EXISTS prevent_remove_project_leader;
                    DROP TRIGGER IF EXISTS sync_direct_grant_on_pm_insert;
                    DROP TRIGGER IF EXISTS sync_grants_on_pm_delete;
                    DROP TRIGGER IF EXISTS sync_pm_after_grant_insert;
                    DROP TRIGGER IF EXISTS sync_pm_after_grant_update;
                    DROP TRIGGER IF EXISTS sync_pm_after_grant_delete;
                    DROP TRIGGER IF EXISTS check_task_author_permission;
                    DROP TRIGGER IF EXISTS check_task_message_permission;
                    DROP TRIGGER IF EXISTS check_assignment_permission;

                    DROP TABLE IF EXISTS project_members_new;
                    CREATE TABLE project_members_new (
                        project_id INTEGER NOT NULL REFERENCES project(id) ON DELETE CASCADE,
                        user_id INTEGER NOT NULL REFERENCES user(id) ON DELETE CASCADE,
                        role TEXT NOT NULL DEFAULT 'member'
                            REFERENCES project_role_definition(role) ON UPDATE CASCADE ON DELETE RESTRICT,
                        permissions_mask INTEGER NOT NULL DEFAULT 15,
                        granted_by_org_role INTEGER NOT NULL DEFAULT 0,
                        active_grant_key TEXT,
                        joined_at INTEGER NOT NULL DEFAULT (unixepoch()),
                        PRIMARY KEY (project_id, user_id)
                    ) WITHOUT ROWID;

                    INSERT OR IGNORE INTO project_members_new (project_id, user_id, role, permissions_mask, granted_by_org_role, active_grant_key, joined_at)
                    SELECT project_id, user_id, role,
                           COALESCE(permissions_mask, 15),
                           COALESCE(granted_by_org_role, 0),
                           active_grant_key,
                           COALESCE(joined_at, unixepoch())
                    FROM project_members;

                    DROP TABLE project_members;
                    ALTER TABLE project_members_new RENAME TO project_members;
                    CREATE INDEX IF NOT EXISTS idx_proj_members_user ON project_members(user_id, project_id);
                    PRAGMA foreign_keys = ON;
                ";
                let _ = sqlx::raw_sql(migration_sql).execute(pool).await;
            }
        }

        let has_perm_mask: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('project_members') WHERE name = 'permissions_mask'"
        )
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        if !has_perm_mask {
            let _ = sqlx::query("ALTER TABLE project_members ADD COLUMN permissions_mask INTEGER NOT NULL DEFAULT 15").execute(pool).await;
        }

        let has_active_grant_key: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('project_members') WHERE name = 'active_grant_key'"
        )
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        if !has_active_grant_key {
            let _ = sqlx::query("ALTER TABLE project_members ADD COLUMN active_grant_key TEXT").execute(pool).await;
        }
    }

    let has_pora_table: bool = sqlx::query_scalar(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='project_org_role_access'"
    )
    .fetch_one(pool)
    .await
    .unwrap_or(false);

    if has_pora_table {
        let has_org_role: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('project_org_role_access') WHERE name = 'org_role'"
        )
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        if !has_org_role {
            let _ = sqlx::query("DROP TABLE IF EXISTS project_org_role_access").execute(pool).await;
        }

        // Clean up any stale direct grant for Bob on Project 1
        let _ = sqlx::query("DELETE FROM project_member_grants WHERE project_id = 1 AND user_id = 2 AND grant_key = 'direct'").execute(pool).await;
    }

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

        CREATE TABLE IF NOT EXISTS organization_member_roles (
            organization_id INTEGER NOT NULL,
            user_id INTEGER NOT NULL,
            role TEXT NOT NULL,
            assigned_at INTEGER NOT NULL DEFAULT (unixepoch()),
            PRIMARY KEY (organization_id, user_id, role),
            FOREIGN KEY (organization_id, user_id)
                REFERENCES organization_members(organization_id, user_id)
                ON DELETE CASCADE,
            FOREIGN KEY (organization_id, role)
                REFERENCES organization_role(organization_id, role)
                ON UPDATE CASCADE ON DELETE RESTRICT
        ) WITHOUT ROWID;

        CREATE INDEX IF NOT EXISTS idx_org_member_roles_lookup ON organization_member_roles(user_id, organization_id);

        CREATE TABLE IF NOT EXISTS organization_governance_transfer (
            organization_id INTEGER PRIMARY KEY REFERENCES organization(id) ON DELETE CASCADE,
            requested_new_governor INTEGER NOT NULL,
            created_at INTEGER NOT NULL DEFAULT (unixepoch()),
            FOREIGN KEY (organization_id, requested_new_governor)
                REFERENCES organization_members(organization_id, user_id) ON DELETE CASCADE
        ) WITHOUT ROWID;

        -- 3. Project Role Definitions & Additive Capability Masks
        CREATE TABLE IF NOT EXISTS project_role_definition (
            role TEXT PRIMARY KEY,
            permissions_mask INTEGER NOT NULL,
            description TEXT NOT NULL DEFAULT ''
        ) WITHOUT ROWID;

        INSERT OR IGNORE INTO project_role_definition (role, permissions_mask, description) VALUES
            ('viewer', 1, 'Read-only access to project tasks and discussions'),
            ('member', 15, 'Create, edit, comment on, and assign tasks'),
            ('admin', 63, 'Full project administration, role management, and task mutations');

        -- 4. Projects & Inter-Organization Affiliation
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

        CREATE TABLE IF NOT EXISTS project_organizations (
            project_id INTEGER NOT NULL REFERENCES project(id) ON DELETE CASCADE,
            organization_id INTEGER NOT NULL REFERENCES organization(id) ON DELETE CASCADE,
            relationship_type TEXT NOT NULL DEFAULT 'partner'
                CHECK (relationship_type IN ('owner', 'partner', 'contractor', 'auditor')),
            max_granted_role TEXT NOT NULL DEFAULT 'member'
                REFERENCES project_role_definition(role) ON UPDATE CASCADE ON DELETE RESTRICT,
            joined_at INTEGER NOT NULL DEFAULT (unixepoch()),
            PRIMARY KEY (project_id, organization_id)
        ) WITHOUT ROWID;

        CREATE TABLE IF NOT EXISTS project_org_role_access (
            project_id INTEGER NOT NULL,
            organization_id INTEGER NOT NULL,
            org_role TEXT NOT NULL,
            project_role TEXT NOT NULL DEFAULT 'member'
                REFERENCES project_role_definition(role) ON UPDATE CASCADE ON DELETE RESTRICT,
            PRIMARY KEY (project_id, organization_id, org_role),
            FOREIGN KEY (project_id, organization_id)
                REFERENCES project_organizations(project_id, organization_id)
                ON DELETE CASCADE,
            FOREIGN KEY (organization_id, org_role)
                REFERENCES organization_role(organization_id, role)
                ON UPDATE CASCADE ON DELETE CASCADE
        ) WITHOUT ROWID;

        CREATE TABLE IF NOT EXISTS project_members (
            project_id INTEGER NOT NULL REFERENCES project(id) ON DELETE CASCADE,
            user_id INTEGER NOT NULL REFERENCES user(id) ON DELETE CASCADE,
            role TEXT NOT NULL DEFAULT 'member'
                REFERENCES project_role_definition(role) ON UPDATE CASCADE ON DELETE RESTRICT,
            permissions_mask INTEGER NOT NULL DEFAULT 15,
            granted_by_org_role INTEGER NOT NULL DEFAULT 0,
            active_grant_key TEXT,
            joined_at INTEGER NOT NULL DEFAULT (unixepoch()),
            PRIMARY KEY (project_id, user_id)
        ) WITHOUT ROWID;

        CREATE INDEX IF NOT EXISTS idx_proj_members_user ON project_members(user_id, project_id);

        CREATE TABLE IF NOT EXISTS project_member_grants (
            project_id INTEGER NOT NULL REFERENCES project(id) ON DELETE CASCADE,
            user_id INTEGER NOT NULL REFERENCES user(id) ON DELETE CASCADE,
            grant_key TEXT NOT NULL,
            project_role TEXT NOT NULL DEFAULT 'member'
                REFERENCES project_role_definition(role) ON UPDATE CASCADE ON DELETE RESTRICT,
            source_org_id INTEGER,
            source_org_role TEXT,
            granted_at INTEGER NOT NULL DEFAULT (unixepoch()),
            PRIMARY KEY (project_id, user_id, grant_key),
            CHECK (
                (grant_key = 'direct' AND source_org_id IS NULL AND source_org_role IS NULL)
                OR (source_org_id IS NOT NULL AND source_org_role IS NOT NULL)
            ),
            FOREIGN KEY (project_id, source_org_id, source_org_role)
                REFERENCES project_org_role_access(project_id, organization_id, org_role)
                ON DELETE CASCADE,
            FOREIGN KEY (source_org_id, user_id, source_org_role)
                REFERENCES organization_member_roles(organization_id, user_id, role)
                ON DELETE CASCADE
        ) WITHOUT ROWID;

        CREATE INDEX IF NOT EXISTS idx_proj_grants_user ON project_member_grants(user_id, project_id);

        CREATE TABLE IF NOT EXISTS project_leadership_transfer (
            project_id INTEGER PRIMARY KEY REFERENCES project(id) ON DELETE CASCADE,
            requested_new_leader INTEGER NOT NULL,
            created_at INTEGER NOT NULL DEFAULT (unixepoch()),
            FOREIGN KEY (project_id, requested_new_leader)
                REFERENCES project_members(project_id, user_id) ON DELETE CASCADE
        ) WITHOUT ROWID;

        -- 5. In-System Invitations & User Message Box
        CREATE TABLE IF NOT EXISTS project_invitation (
            project_id INTEGER NOT NULL REFERENCES project(id) ON DELETE CASCADE,
            invited_user_id INTEGER NOT NULL REFERENCES user(id) ON DELETE CASCADE,
            invited_by INTEGER REFERENCES user(id) ON DELETE SET NULL,
            role TEXT NOT NULL DEFAULT 'member' CHECK (role IN ('admin', 'member', 'viewer')),
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

        -- 6. Tasks, Assignments & Discussion Threads
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
            SELECT RAISE(ABORT, 'Cannot delete built-in organization roles (admin, member).');
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
            SELECT RAISE(ABORT, 'Cannot delete an organization while other members remain. Remove all other members first.');
        END;

        -- Organization Guardrail: Cannot remove the governing user from organization_members
        CREATE TRIGGER IF NOT EXISTS prevent_remove_governing_user
        BEFORE DELETE ON organization_members
        FOR EACH ROW
        WHEN (SELECT governing_user_id FROM organization WHERE id = OLD.organization_id) = OLD.user_id
        BEGIN
            SELECT RAISE(ABORT, 'The governing user cannot be removed from the organization. Transfer governance first.');
        END;

        -- Organization Member Sync: Auto-seed initial role into organization_member_roles
        CREATE TRIGGER IF NOT EXISTS sync_org_member_roles_after_member_insert
        AFTER INSERT ON organization_members
        FOR EACH ROW
        BEGIN
            INSERT OR IGNORE INTO organization_member_roles (organization_id, user_id, role, assigned_at)
            VALUES (NEW.organization_id, NEW.user_id, NEW.role, NEW.joined_at);
        END;

        -- Organization Member Sync: Update organization_member_roles on role update
        CREATE TRIGGER IF NOT EXISTS sync_org_member_roles_after_member_update
        AFTER UPDATE OF role ON organization_members
        FOR EACH ROW
        WHEN OLD.role != NEW.role
        BEGIN
            DELETE FROM organization_member_roles
            WHERE organization_id = OLD.organization_id
              AND user_id = OLD.user_id
              AND role = OLD.role;

            INSERT OR IGNORE INTO organization_member_roles (organization_id, user_id, role, assigned_at)
            VALUES (NEW.organization_id, NEW.user_id, NEW.role, unixepoch());
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
            SELECT RAISE(ABORT, 'Cannot delete a project while other members remain. Remove all other members first.');
        END;

        -- Project Guardrail: Cannot remove the project leader from project_members
        CREATE TRIGGER IF NOT EXISTS prevent_remove_project_leader
        BEFORE DELETE ON project_members
        FOR EACH ROW
        WHEN (SELECT leader_id FROM project WHERE id = OLD.project_id) = OLD.user_id
        BEGIN
            SELECT RAISE(ABORT, 'The project leader cannot be removed from the project. Transfer project leadership first.');
        END;

        -- Project Org Owner Sync: When a project is created with owner_org_id, affiliate the org as owner/admin
        CREATE TRIGGER IF NOT EXISTS sync_project_owner_org_insert
        AFTER INSERT ON project
        FOR EACH ROW
        WHEN NEW.owner_org_id IS NOT NULL
        BEGIN
            INSERT OR IGNORE INTO project_organizations (project_id, organization_id, relationship_type, max_granted_role)
            VALUES (NEW.id, NEW.owner_org_id, 'owner', 'admin');
        END;

        -- Ceiling Check: Privilege ceiling enforcement on project_org_role_access insertion
        CREATE TRIGGER IF NOT EXISTS check_org_project_role_ceiling_insert
        BEFORE INSERT ON project_org_role_access
        FOR EACH ROW
        WHEN (
            SELECT (prd_grant.permissions_mask & ~prd_ceiling.permissions_mask)
            FROM project_role_definition prd_grant,
                 project_organizations po
                 JOIN project_role_definition prd_ceiling ON prd_ceiling.role = po.max_granted_role
            WHERE prd_grant.role = NEW.project_role
              AND po.project_id = NEW.project_id
              AND po.organization_id = NEW.organization_id
        ) != 0
        BEGIN
            SELECT RAISE(ABORT, 'Granted project role exceeds the organization privilege ceiling for this project.');
        END;

        -- Ceiling Check: Privilege ceiling enforcement on project_org_role_access update
        CREATE TRIGGER IF NOT EXISTS check_org_project_role_ceiling_update
        BEFORE UPDATE OF project_role ON project_org_role_access
        FOR EACH ROW
        WHEN (
            SELECT (prd_grant.permissions_mask & ~prd_ceiling.permissions_mask)
            FROM project_role_definition prd_grant,
                 project_organizations po
                 JOIN project_role_definition prd_ceiling ON prd_ceiling.role = po.max_granted_role
            WHERE prd_grant.role = NEW.project_role
              AND po.project_id = NEW.project_id
              AND po.organization_id = NEW.organization_id
        ) != 0
        BEGIN
            SELECT RAISE(ABORT, 'Granted project role exceeds the organization privilege ceiling for this project.');
        END;

        -- Ceiling Check: Privilege ceiling enforcement when max_granted_role is updated on project_organizations
        CREATE TRIGGER IF NOT EXISTS check_org_max_role_ceiling_update
        BEFORE UPDATE OF max_granted_role ON project_organizations
        FOR EACH ROW
        WHEN EXISTS (
            SELECT 1 FROM project_org_role_access pora
            JOIN project_role_definition prd_grant ON prd_grant.role = pora.project_role
            JOIN project_role_definition prd_ceiling ON prd_ceiling.role = NEW.max_granted_role
            WHERE pora.project_id = NEW.project_id
              AND pora.organization_id = NEW.organization_id
              AND (prd_grant.permissions_mask & ~prd_ceiling.permissions_mask) != 0
        )
        BEGIN
            SELECT RAISE(ABORT, 'Cannot lower organization privilege ceiling below existing role access rules. Update role rules first.');
        END;

        -- Materialization: Synchronize grants when a rule is added to project_org_role_access
        CREATE TRIGGER IF NOT EXISTS sync_grants_after_rule_insert
        AFTER INSERT ON project_org_role_access
        FOR EACH ROW
        BEGIN
            INSERT OR REPLACE INTO project_member_grants (
                project_id, user_id, grant_key, project_role, source_org_id, source_org_role, granted_at
            )
            SELECT
                NEW.project_id,
                omr.user_id,
                'org:' || NEW.organization_id || ':' || NEW.org_role,
                NEW.project_role,
                NEW.organization_id,
                NEW.org_role,
                unixepoch()
            FROM organization_member_roles omr
            WHERE omr.organization_id = NEW.organization_id
              AND omr.role = NEW.org_role;
        END;

        -- Materialization: Synchronize grants when a rule's project_role is updated in project_org_role_access
        CREATE TRIGGER IF NOT EXISTS sync_grants_after_rule_update
        AFTER UPDATE OF project_role ON project_org_role_access
        FOR EACH ROW
        BEGIN
            UPDATE project_member_grants
            SET project_role = NEW.project_role
            WHERE project_id = NEW.project_id
              AND source_org_id = NEW.organization_id
              AND source_org_role = NEW.org_role;
        END;

        -- Materialization: Synchronize grants when a rule is removed from project_org_role_access
        DROP TRIGGER IF EXISTS sync_grants_after_rule_delete;
        CREATE TRIGGER sync_grants_after_rule_delete
        AFTER DELETE ON project_org_role_access
        FOR EACH ROW
        BEGIN
            DELETE FROM project_member_grants
            WHERE project_id = OLD.project_id
              AND source_org_id = OLD.organization_id
              AND source_org_role = OLD.org_role;

            DELETE FROM project_members
            WHERE project_id = OLD.project_id
              AND NOT EXISTS (
                  SELECT 1 FROM project_member_grants
                  WHERE project_id = OLD.project_id AND user_id = project_members.user_id
              );

            UPDATE project_members
            SET
                permissions_mask = (
                    SELECT COALESCE(
                        (SELECT prd.permissions_mask
                         FROM project_member_grants pmg
                         JOIN project_role_definition prd ON prd.role = pmg.project_role
                         WHERE pmg.project_id = OLD.project_id
                           AND pmg.user_id = project_members.user_id
                           AND pmg.grant_key = project_members.active_grant_key),
                        (SELECT (MAX(prd.permissions_mask & 1))
                              | (MAX(prd.permissions_mask & 2))
                              | (MAX(prd.permissions_mask & 4))
                              | (MAX(prd.permissions_mask & 8))
                              | (MAX(prd.permissions_mask & 16))
                              | (MAX(prd.permissions_mask & 32))
                         FROM project_member_grants pmg
                         JOIN project_role_definition prd ON prd.role = pmg.project_role
                         WHERE pmg.project_id = OLD.project_id AND pmg.user_id = project_members.user_id)
                    )
                ),
                granted_by_org_role = CASE
                    WHEN EXISTS (
                        SELECT 1 FROM project_member_grants
                        WHERE project_id = OLD.project_id AND user_id = project_members.user_id AND grant_key = 'direct'
                    ) THEN 0
                    ELSE 1
                END
            WHERE project_id = OLD.project_id;

            UPDATE project_members
            SET role = CASE
                WHEN permissions_mask & 32 != 0 THEN 'admin'
                WHEN permissions_mask & 4 != 0 THEN 'member'
                ELSE 'viewer'
            END
            WHERE project_id = OLD.project_id;
        END;

        -- Materialization: Synchronize grants when a user gains a role in organization_member_roles
        CREATE TRIGGER IF NOT EXISTS sync_grants_after_org_role_insert
        AFTER INSERT ON organization_member_roles
        FOR EACH ROW
        BEGIN
            INSERT OR REPLACE INTO project_member_grants (
                project_id, user_id, grant_key, project_role, source_org_id, source_org_role, granted_at
            )
            SELECT
                pora.project_id,
                NEW.user_id,
                'org:' || NEW.organization_id || ':' || NEW.role,
                pora.project_role,
                NEW.organization_id,
                NEW.role,
                unixepoch()
            FROM project_org_role_access pora
            WHERE pora.organization_id = NEW.organization_id
              AND pora.org_role = NEW.role;
        END;

        -- Materialization: Synchronize grants when a user loses a role in organization_member_roles
        DROP TRIGGER IF EXISTS sync_grants_after_org_role_delete;
        CREATE TRIGGER sync_grants_after_org_role_delete
        AFTER DELETE ON organization_member_roles
        FOR EACH ROW
        BEGIN
            DELETE FROM project_member_grants
            WHERE source_org_id = OLD.organization_id
              AND user_id = OLD.user_id
              AND source_org_role = OLD.role;

            DELETE FROM project_members
            WHERE user_id = OLD.user_id
              AND NOT EXISTS (
                  SELECT 1 FROM project_member_grants
                  WHERE project_id = project_members.project_id AND user_id = OLD.user_id
              );

            UPDATE project_members
            SET
                permissions_mask = (
                    SELECT COALESCE(
                        (SELECT prd.permissions_mask
                         FROM project_member_grants pmg
                         JOIN project_role_definition prd ON prd.role = pmg.project_role
                         WHERE pmg.project_id = project_members.project_id
                           AND pmg.user_id = OLD.user_id
                           AND pmg.grant_key = project_members.active_grant_key),
                        (SELECT (MAX(prd.permissions_mask & 1))
                              | (MAX(prd.permissions_mask & 2))
                              | (MAX(prd.permissions_mask & 4))
                              | (MAX(prd.permissions_mask & 8))
                              | (MAX(prd.permissions_mask & 16))
                              | (MAX(prd.permissions_mask & 32))
                         FROM project_member_grants pmg
                         JOIN project_role_definition prd ON prd.role = pmg.project_role
                         WHERE pmg.project_id = project_members.project_id AND pmg.user_id = OLD.user_id)
                    )
                ),
                granted_by_org_role = CASE
                    WHEN EXISTS (
                        SELECT 1 FROM project_member_grants
                        WHERE project_id = project_members.project_id AND user_id = OLD.user_id AND grant_key = 'direct'
                    ) THEN 0
                    ELSE 1
                END
            WHERE user_id = OLD.user_id;

            UPDATE project_members
            SET role = CASE
                WHEN permissions_mask & 32 != 0 THEN 'admin'
                WHEN permissions_mask & 4 != 0 THEN 'member'
                ELSE 'viewer'
            END
            WHERE user_id = OLD.user_id;
        END;

        -- Materialization: Synchronize grants when a user is removed from organization_members
        DROP TRIGGER IF EXISTS sync_grants_after_om_delete;
        CREATE TRIGGER sync_grants_after_om_delete
        AFTER DELETE ON organization_members
        FOR EACH ROW
        BEGIN
            DELETE FROM organization_member_roles
            WHERE organization_id = OLD.organization_id AND user_id = OLD.user_id;

            DELETE FROM project_member_grants
            WHERE source_org_id = OLD.organization_id AND user_id = OLD.user_id;

            DELETE FROM project_members
            WHERE user_id = OLD.user_id
              AND NOT EXISTS (
                  SELECT 1 FROM project_member_grants
                  WHERE project_id = project_members.project_id AND user_id = OLD.user_id
              );
        END;

        -- Materialization: Sync project_members when a grant is inserted
        CREATE TRIGGER IF NOT EXISTS sync_pm_after_grant_insert
        AFTER INSERT ON project_member_grants
        FOR EACH ROW
        BEGIN
            INSERT INTO project_members (
                project_id, user_id, role, permissions_mask, granted_by_org_role, active_grant_key, joined_at
            )
            VALUES (
                NEW.project_id,
                NEW.user_id,
                NEW.project_role,
                (SELECT permissions_mask FROM project_role_definition WHERE role = NEW.project_role),
                CASE WHEN NEW.grant_key = 'direct' THEN 0 ELSE 1 END,
                NULL,
                unixepoch()
            )
            ON CONFLICT(project_id, user_id) DO UPDATE SET
                permissions_mask = (
                    SELECT COALESCE(
                        (SELECT prd.permissions_mask
                         FROM project_member_grants pmg
                         JOIN project_role_definition prd ON prd.role = pmg.project_role
                         WHERE pmg.project_id = NEW.project_id
                           AND pmg.user_id = NEW.user_id
                           AND pmg.grant_key = project_members.active_grant_key),
                        (SELECT (MAX(prd.permissions_mask & 1))
                              | (MAX(prd.permissions_mask & 2))
                              | (MAX(prd.permissions_mask & 4))
                              | (MAX(prd.permissions_mask & 8))
                              | (MAX(prd.permissions_mask & 16))
                              | (MAX(prd.permissions_mask & 32))
                         FROM project_member_grants pmg
                         JOIN project_role_definition prd ON prd.role = pmg.project_role
                         WHERE pmg.project_id = NEW.project_id AND pmg.user_id = NEW.user_id)
                    )
                ),
                granted_by_org_role = CASE
                    WHEN EXISTS (
                        SELECT 1 FROM project_member_grants
                        WHERE project_id = NEW.project_id AND user_id = NEW.user_id AND grant_key = 'direct'
                    ) THEN 0
                    ELSE 1
                END;

            UPDATE project_members
            SET role = CASE
                WHEN permissions_mask & 32 != 0 THEN 'admin'
                WHEN permissions_mask & 4 != 0 THEN 'member'
                ELSE 'viewer'
            END
            WHERE project_id = NEW.project_id AND user_id = NEW.user_id;
        END;

        -- Materialization: Sync project_members when a grant is updated
        CREATE TRIGGER IF NOT EXISTS sync_pm_after_grant_update
        AFTER UPDATE ON project_member_grants
        FOR EACH ROW
        BEGIN
            UPDATE project_members
            SET
                permissions_mask = (
                    SELECT COALESCE(
                        (SELECT prd.permissions_mask
                         FROM project_member_grants pmg
                         JOIN project_role_definition prd ON prd.role = pmg.project_role
                         WHERE pmg.project_id = NEW.project_id
                           AND pmg.user_id = NEW.user_id
                           AND pmg.grant_key = project_members.active_grant_key),
                        (SELECT (MAX(prd.permissions_mask & 1))
                              | (MAX(prd.permissions_mask & 2))
                              | (MAX(prd.permissions_mask & 4))
                              | (MAX(prd.permissions_mask & 8))
                              | (MAX(prd.permissions_mask & 16))
                              | (MAX(prd.permissions_mask & 32))
                         FROM project_member_grants pmg
                         JOIN project_role_definition prd ON prd.role = pmg.project_role
                         WHERE pmg.project_id = NEW.project_id AND pmg.user_id = NEW.user_id)
                    )
                ),
                granted_by_org_role = CASE
                    WHEN EXISTS (
                        SELECT 1 FROM project_member_grants
                        WHERE project_id = NEW.project_id AND user_id = NEW.user_id AND grant_key = 'direct'
                    ) THEN 0
                    ELSE 1
                END
            WHERE project_id = NEW.project_id AND user_id = NEW.user_id;

            UPDATE project_members
            SET role = CASE
                WHEN permissions_mask & 32 != 0 THEN 'admin'
                WHEN permissions_mask & 4 != 0 THEN 'member'
                ELSE 'viewer'
            END
            WHERE project_id = NEW.project_id AND user_id = NEW.user_id;
        END;

        -- Materialization: Sync project_members when a grant is deleted
        CREATE TRIGGER IF NOT EXISTS sync_pm_after_grant_delete
        AFTER DELETE ON project_member_grants
        FOR EACH ROW
        BEGIN
            DELETE FROM project_members
            WHERE project_id = OLD.project_id
              AND user_id = OLD.user_id
              AND NOT EXISTS (
                  SELECT 1 FROM project_member_grants
                  WHERE project_id = OLD.project_id AND user_id = OLD.user_id
              );

            UPDATE project_members
            SET
                active_grant_key = CASE
                    WHEN active_grant_key = OLD.grant_key THEN NULL
                    ELSE active_grant_key
                END,
                permissions_mask = (
                    SELECT COALESCE(
                        (SELECT prd.permissions_mask
                         FROM project_member_grants pmg
                         JOIN project_role_definition prd ON prd.role = pmg.project_role
                         WHERE pmg.project_id = OLD.project_id
                           AND pmg.user_id = OLD.user_id
                           AND pmg.grant_key = project_members.active_grant_key
                           AND pmg.grant_key != OLD.grant_key),
                        (SELECT (MAX(prd.permissions_mask & 1))
                              | (MAX(prd.permissions_mask & 2))
                              | (MAX(prd.permissions_mask & 4))
                              | (MAX(prd.permissions_mask & 8))
                              | (MAX(prd.permissions_mask & 16))
                              | (MAX(prd.permissions_mask & 32))
                         FROM project_member_grants pmg
                         JOIN project_role_definition prd ON prd.role = pmg.project_role
                         WHERE pmg.project_id = OLD.project_id AND pmg.user_id = OLD.user_id)
                    )
                ),
                granted_by_org_role = CASE
                    WHEN EXISTS (
                        SELECT 1 FROM project_member_grants
                        WHERE project_id = OLD.project_id AND user_id = OLD.user_id AND grant_key = 'direct'
                    ) THEN 0
                    ELSE 1
                END
            WHERE project_id = OLD.project_id AND user_id = OLD.user_id;

            UPDATE project_members
            SET role = CASE
                WHEN permissions_mask & 32 != 0 THEN 'admin'
                WHEN permissions_mask & 4 != 0 THEN 'member'
                ELSE 'viewer'
            END
            WHERE project_id = OLD.project_id AND user_id = OLD.user_id;
        END;

        -- Direct Member Sync: When inserted directly into project_members, ensure grant ledger records 'direct'
        CREATE TRIGGER IF NOT EXISTS sync_direct_grant_on_pm_insert
        AFTER INSERT ON project_members
        FOR EACH ROW
        WHEN NOT EXISTS (SELECT 1 FROM project_member_grants WHERE project_id = NEW.project_id AND user_id = NEW.user_id)
        BEGIN
            INSERT OR IGNORE INTO project_member_grants (
                project_id, user_id, grant_key, project_role, source_org_id, source_org_role, granted_at
            ) VALUES (
                NEW.project_id, NEW.user_id, 'direct', NEW.role, NULL, NULL, NEW.joined_at
            );
        END;

        -- Direct Member Sync: When deleted from project_members, remove all grants for that user on that project
        -- and clean up active assignments on this project's tasks
        DROP TRIGGER IF EXISTS sync_grants_on_pm_delete;
        CREATE TRIGGER sync_grants_on_pm_delete
        AFTER DELETE ON project_members
        FOR EACH ROW
        BEGIN
            DELETE FROM project_member_grants
            WHERE project_id = OLD.project_id AND user_id = OLD.user_id;

            -- Revert any in_progress task to accepted if this user is the only assignee
            UPDATE task
            SET status = 'accepted'
            WHERE project_id = OLD.project_id
              AND status = 'in_progress'
              AND (SELECT COUNT(*) FROM assignments WHERE task_id = task.id) <= 1
              AND EXISTS (SELECT 1 FROM assignments WHERE task_id = task.id AND assigned_to = OLD.user_id);

            -- Remove user assignments on this project's tasks
            DELETE FROM assignments
            WHERE assigned_to = OLD.user_id
              AND task_id IN (SELECT id FROM task WHERE project_id = OLD.project_id);
        END;

        -- Assignment Cleanup on Permission Demotion: When member loses PERM_BE_ASSIGNED (4), purge assignments
        DROP TRIGGER IF EXISTS cleanup_assignments_on_pm_demote;
        CREATE TRIGGER cleanup_assignments_on_pm_demote
        AFTER UPDATE OF permissions_mask ON project_members
        FOR EACH ROW
        WHEN (NEW.permissions_mask & 4) = 0 AND (OLD.permissions_mask & 4) != 0
        BEGIN
            -- Revert in_progress task to accepted if demoted user is sole assignee
            UPDATE task
            SET status = 'accepted'
            WHERE project_id = NEW.project_id
              AND status = 'in_progress'
              AND (SELECT COUNT(*) FROM assignments WHERE task_id = task.id) <= 1
              AND EXISTS (SELECT 1 FROM assignments WHERE task_id = task.id AND assigned_to = NEW.user_id);

            -- Remove user assignments on this project's tasks
            DELETE FROM assignments
            WHERE assigned_to = NEW.user_id
              AND task_id IN (SELECT id FROM task WHERE project_id = NEW.project_id);
        END;

        -- Task Trigger 1: Cannot assign a task that is in 'suggested' status
        CREATE TRIGGER IF NOT EXISTS prevent_assign_suggested_task
        BEFORE INSERT ON assignments
        FOR EACH ROW
        WHEN (SELECT status FROM task WHERE id = NEW.task_id) = 'suggested'
        BEGIN
            SELECT RAISE(ABORT, 'Cannot assign a task that has suggested status');
        END;

        -- Task Trigger 2: Cannot change task status to 'in_progress' without at least one assignee
        CREATE TRIGGER IF NOT EXISTS enforce_in_progress_assigned
        BEFORE UPDATE OF status ON task
        FOR EACH ROW
        WHEN NEW.status = 'in_progress' AND NOT EXISTS (SELECT 1 FROM assignments WHERE task_id = NEW.id)
        BEGIN
            SELECT RAISE(ABORT, 'Cannot set status to in_progress without an assignee');
        END;

        -- Task Trigger 3: Cannot remove the last assignee if task is 'in_progress'
        CREATE TRIGGER IF NOT EXISTS prevent_unassign_in_progress_task
        BEFORE DELETE ON assignments
        FOR EACH ROW
        WHEN (SELECT status FROM task WHERE id = OLD.task_id) = 'in_progress' 
             AND (SELECT COUNT(*) FROM assignments WHERE task_id = OLD.task_id) <= 1
        BEGIN
            SELECT RAISE(ABORT, 'Cannot remove the last assignee from an in_progress task');
        END;

        -- Task Authorization Trigger: Author must have PERM_EDIT_TASKS (4)
        CREATE TRIGGER IF NOT EXISTS check_task_author_permission
        BEFORE INSERT ON task
        FOR EACH ROW
        WHEN NEW.project_id IS NOT NULL
          AND NEW.author IS NOT NULL
          AND NOT EXISTS (
              SELECT 1 FROM project_members
              WHERE project_id = NEW.project_id
                AND user_id = NEW.author
                AND (permissions_mask & 4) != 0
          )
        BEGIN
            SELECT RAISE(ABORT, 'User does not have permission to create or edit tasks in this project.');
        END;

        -- Task Discussion Trigger: Author must have PERM_COMMENT (2)
        CREATE TRIGGER IF NOT EXISTS check_task_message_permission
        BEFORE INSERT ON task_message
        FOR EACH ROW
        WHEN (SELECT project_id FROM task WHERE id = NEW.task_id) IS NOT NULL
          AND NEW.author_id IS NOT NULL
          AND NOT EXISTS (
              SELECT 1 FROM project_members
              WHERE project_id = (SELECT project_id FROM task WHERE id = NEW.task_id)
                AND user_id = NEW.author_id
                AND (permissions_mask & 2) != 0
          )
        BEGIN
            SELECT RAISE(ABORT, 'User does not have permission to post messages in this project.');
        END;

        -- Task Assignment Authorization Trigger: Assignee must have PERM_EDIT_TASKS (4)
        CREATE TRIGGER IF NOT EXISTS check_assignment_permission
        BEFORE INSERT ON assignments
        FOR EACH ROW
        WHEN (SELECT project_id FROM task WHERE id = NEW.task_id) IS NOT NULL
          AND NOT EXISTS (
            SELECT 1 FROM project_members
            WHERE project_id = (SELECT project_id FROM task WHERE id = NEW.task_id)
              AND user_id = NEW.assigned_to
              AND (permissions_mask & 4) != 0
        )
        BEGIN
            SELECT RAISE(ABORT, 'Cannot assign task to a user who lacks task execution permissions.');
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

        -- Organization role mappings for Acme Engineering (members and admins have access)
        INSERT OR IGNORE INTO project_org_role_access (project_id, organization_id, org_role, project_role)
        VALUES (1, 1, 'admin', 'admin'), (1, 1, 'member', 'member');

        -- Project 1 leader: Alice (admin, direct)
        INSERT OR IGNORE INTO project_members (project_id, user_id, role, permissions_mask, granted_by_org_role)
        VALUES (1, 1, 'admin', 63, 0);

        INSERT OR IGNORE INTO project_member_grants (project_id, user_id, grant_key, project_role)
        VALUES (1, 1, 'direct', 'admin');

        -- Default Project 2: Bob''s Personal Project (Standalone, leader: Bob)
        INSERT OR IGNORE INTO project (id, name, description, owner_org_id, leader_id)
        VALUES (2, 'Personal Roadmap', 'Standalone project roadmap and personal tasks.', NULL, 2);

        -- Project 2 members: Bob (admin)
        INSERT OR IGNORE INTO project_members (project_id, user_id, role, permissions_mask, granted_by_org_role)
        VALUES (2, 2, 'admin', 63, 0);

        INSERT OR IGNORE INTO project_member_grants (project_id, user_id, grant_key, project_role)
        VALUES (2, 2, 'direct', 'admin');

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

    Ok(())
}