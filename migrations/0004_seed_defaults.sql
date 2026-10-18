-- 0004_seed_defaults.sql
-- Default reference seed data: users, organizations, projects, access rules, tasks, and sequence sync

-- 1. Default Users
INSERT INTO "user" (id, name, email, password_hash) VALUES
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
    (15, 'George Best', 'george.best@manchester.united.com', '')
ON CONFLICT (id) DO NOTHING;

-- 2. Default Organization (Acme Corp)
INSERT INTO organization (id, name, slug, governing_user_id)
VALUES (1, 'Acme Corp', 'acme-corp', 1)
ON CONFLICT (id) DO NOTHING;

INSERT INTO organization_members (organization_id, user_id, role)
VALUES (1, 1, 'admin'), (1, 2, 'member')
ON CONFLICT (organization_id, user_id) DO NOTHING;

-- 3. Default Projects
INSERT INTO project (id, name, description, owner_org_id, leader_id)
VALUES
    (1, 'Acme Engineering', 'Core product development and backend services.', 1, 1),
    (2, 'Personal Roadmap', 'Standalone project roadmap and personal tasks.', NULL, 2)
ON CONFLICT (id) DO NOTHING;

INSERT INTO project_org_role_access (project_id, organization_id, org_role, project_role)
VALUES (1, 1, 'admin', 'admin'), (1, 1, 'member', 'member')
ON CONFLICT (project_id, organization_id, org_role) DO NOTHING;

INSERT INTO project_members (project_id, user_id, role, permissions_mask, granted_by_org_role)
VALUES
    (1, 1, 'admin', 63, 0),
    (2, 2, 'admin', 63, 0)
ON CONFLICT (project_id, user_id) DO NOTHING;

INSERT INTO project_member_grants (project_id, user_id, grant_key, project_role)
VALUES
    (1, 1, 'direct', 'admin'),
    (2, 2, 'direct', 'admin')
ON CONFLICT (project_id, user_id, grant_key) DO NOTHING;

-- 4. Initial Tasks
INSERT INTO task (id, project_id, title, description, author, status) VALUES
    (1, 1, 'Refactor Authentication Pipeline', 'Migrate auth guards to Axum session pool and role checks.', 1, 'assigned'),
    (2, 1, 'Implement Loro CRDT Collaboration', 'Design ephemeral collaboration worker for real-time document editing.', 1, 'suggested'),
    (3, 1, 'Setup Automated CI/CD', 'Deploy staging pipeline with PostgreSQL and automated smoke tests.', 2, 'accepted')
ON CONFLICT (id) DO NOTHING;

INSERT INTO assignments (task_id, assigned_to) VALUES
    (1, 1),
    (1, 2)
ON CONFLICT (task_id, assigned_to) DO NOTHING;

INSERT INTO task_message (task_id, msg_id, author_id, created_at, msg) VALUES
    (1, 1, 1, EXTRACT(EPOCH FROM NOW())::BIGINT - 3600, 'I started moving the endpoints into backend/app_endpoints.rs.'),
    (1, 2, 2, EXTRACT(EPOCH FROM NOW())::BIGINT - 1800, 'Looks great! I verified the session pool handles cookies correctly.')
ON CONFLICT (task_id, msg_id) DO NOTHING;

-- 5. Synchronize BIGSERIAL sequences with seeded IDs
SELECT setval(pg_get_serial_sequence('"user"', 'id'), COALESCE((SELECT MAX(id) FROM "user"), 1), true);
SELECT setval(pg_get_serial_sequence('organization', 'id'), COALESCE((SELECT MAX(id) FROM organization), 1), true);
SELECT setval(pg_get_serial_sequence('project', 'id'), COALESCE((SELECT MAX(id) FROM project), 1), true);
SELECT setval(pg_get_serial_sequence('task', 'id'), COALESCE((SELECT MAX(id) FROM task), 1), true);

