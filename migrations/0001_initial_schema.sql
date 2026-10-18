-- 0001_initial_schema.sql
-- Base tables, constraints, indexes, and static definitions

CREATE TABLE IF NOT EXISTS "user" (
    id BIGSERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    email TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS session (
    token TEXT PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES "user"(id) ON DELETE CASCADE,
    created_at BIGINT NOT NULL,
    expires_at BIGINT NOT NULL
);

CREATE TABLE IF NOT EXISTS organization (
    id BIGSERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    governing_user_id BIGINT NOT NULL REFERENCES "user"(id) ON DELETE RESTRICT,
    created_at BIGINT NOT NULL DEFAULT (EXTRACT(EPOCH FROM NOW())::BIGINT)
);

CREATE TABLE IF NOT EXISTS organization_role (
    organization_id BIGINT NOT NULL REFERENCES organization(id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    PRIMARY KEY (organization_id, role)
);

CREATE TABLE IF NOT EXISTS organization_members (
    organization_id BIGINT NOT NULL REFERENCES organization(id) ON DELETE CASCADE,
    user_id BIGINT NOT NULL REFERENCES "user"(id) ON DELETE CASCADE,
    role TEXT NOT NULL DEFAULT 'member',
    joined_at BIGINT NOT NULL DEFAULT (EXTRACT(EPOCH FROM NOW())::BIGINT),
    PRIMARY KEY (organization_id, user_id),
    FOREIGN KEY (organization_id, role)
        REFERENCES organization_role(organization_id, role)
        ON UPDATE CASCADE ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_org_members_user ON organization_members(user_id, organization_id);
CREATE INDEX IF NOT EXISTS idx_org_members_role ON organization_members(organization_id, role);

CREATE TABLE IF NOT EXISTS organization_member_roles (
    organization_id BIGINT NOT NULL,
    user_id BIGINT NOT NULL,
    role TEXT NOT NULL,
    assigned_at BIGINT NOT NULL DEFAULT (EXTRACT(EPOCH FROM NOW())::BIGINT),
    PRIMARY KEY (organization_id, user_id, role),
    FOREIGN KEY (organization_id, user_id)
        REFERENCES organization_members(organization_id, user_id)
        ON DELETE CASCADE,
    FOREIGN KEY (organization_id, role)
        REFERENCES organization_role(organization_id, role)
        ON UPDATE CASCADE ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_org_member_roles_lookup ON organization_member_roles(user_id, organization_id);

CREATE TABLE IF NOT EXISTS organization_governance_transfer (
    organization_id BIGINT PRIMARY KEY REFERENCES organization(id) ON DELETE CASCADE,
    requested_new_governor BIGINT NOT NULL,
    created_at BIGINT NOT NULL DEFAULT (EXTRACT(EPOCH FROM NOW())::BIGINT),
    FOREIGN KEY (organization_id, requested_new_governor)
        REFERENCES organization_members(organization_id, user_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS project_role_definition (
    role TEXT PRIMARY KEY,
    permissions_mask BIGINT NOT NULL,
    description TEXT NOT NULL DEFAULT ''
);

INSERT INTO project_role_definition (role, permissions_mask, description) VALUES
    ('viewer', 1, 'Read-only access to project tasks and discussions'),
    ('member', 15, 'Create, edit, comment on, and assign tasks'),
    ('admin', 63, 'Full project administration, role management, and task mutations')
ON CONFLICT (role) DO NOTHING;

CREATE TABLE IF NOT EXISTS project (
    id BIGSERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    owner_org_id BIGINT REFERENCES organization(id) ON DELETE RESTRICT,
    leader_id BIGINT NOT NULL REFERENCES "user"(id) ON DELETE RESTRICT,
    created_at BIGINT NOT NULL DEFAULT (EXTRACT(EPOCH FROM NOW())::BIGINT)
);

CREATE INDEX IF NOT EXISTS idx_project_owner_org ON project(owner_org_id) WHERE owner_org_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_project_leader ON project(leader_id);

CREATE TABLE IF NOT EXISTS project_organizations (
    project_id BIGINT NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    organization_id BIGINT NOT NULL REFERENCES organization(id) ON DELETE CASCADE,
    relationship_type TEXT NOT NULL DEFAULT 'partner'
        CHECK (relationship_type IN ('owner', 'partner', 'contractor', 'auditor')),
    max_granted_role TEXT NOT NULL DEFAULT 'member'
        REFERENCES project_role_definition(role) ON UPDATE CASCADE ON DELETE RESTRICT,
    joined_at BIGINT NOT NULL DEFAULT (EXTRACT(EPOCH FROM NOW())::BIGINT),
    PRIMARY KEY (project_id, organization_id)
);

CREATE TABLE IF NOT EXISTS project_org_role_access (
    project_id BIGINT NOT NULL,
    organization_id BIGINT NOT NULL,
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
);

CREATE TABLE IF NOT EXISTS project_members (
    project_id BIGINT NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    user_id BIGINT NOT NULL REFERENCES "user"(id) ON DELETE CASCADE,
    role TEXT NOT NULL DEFAULT 'member'
        REFERENCES project_role_definition(role) ON UPDATE CASCADE ON DELETE RESTRICT,
    permissions_mask BIGINT NOT NULL DEFAULT 15,
    granted_by_org_role BIGINT NOT NULL DEFAULT 0,
    active_grant_key TEXT,
    joined_at BIGINT NOT NULL DEFAULT (EXTRACT(EPOCH FROM NOW())::BIGINT),
    PRIMARY KEY (project_id, user_id)
);

CREATE INDEX IF NOT EXISTS idx_proj_members_user ON project_members(user_id, project_id);

CREATE TABLE IF NOT EXISTS project_member_grants (
    project_id BIGINT NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    user_id BIGINT NOT NULL REFERENCES "user"(id) ON DELETE CASCADE,
    grant_key TEXT NOT NULL,
    project_role TEXT NOT NULL DEFAULT 'member'
        REFERENCES project_role_definition(role) ON UPDATE CASCADE ON DELETE RESTRICT,
    source_org_id BIGINT,
    source_org_role TEXT,
    granted_at BIGINT NOT NULL DEFAULT (EXTRACT(EPOCH FROM NOW())::BIGINT),
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
);

CREATE INDEX IF NOT EXISTS idx_proj_grants_user ON project_member_grants(user_id, project_id);

CREATE TABLE IF NOT EXISTS project_leadership_transfer (
    project_id BIGINT PRIMARY KEY REFERENCES project(id) ON DELETE CASCADE,
    requested_new_leader BIGINT NOT NULL,
    created_at BIGINT NOT NULL DEFAULT (EXTRACT(EPOCH FROM NOW())::BIGINT),
    FOREIGN KEY (project_id, requested_new_leader)
        REFERENCES project_members(project_id, user_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS project_invitation (
    project_id BIGINT NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    invited_user_id BIGINT NOT NULL REFERENCES "user"(id) ON DELETE CASCADE,
    invited_by BIGINT REFERENCES "user"(id) ON DELETE SET NULL,
    role TEXT NOT NULL DEFAULT 'member' CHECK (role IN ('admin', 'member', 'viewer')),
    message TEXT NOT NULL DEFAULT '',
    created_at BIGINT NOT NULL DEFAULT (EXTRACT(EPOCH FROM NOW())::BIGINT),
    PRIMARY KEY (project_id, invited_user_id)
);

CREATE INDEX IF NOT EXISTS idx_proj_invite_user ON project_invitation(invited_user_id);

CREATE TABLE IF NOT EXISTS organization_invitation (
    organization_id BIGINT NOT NULL REFERENCES organization(id) ON DELETE CASCADE,
    invited_user_id BIGINT NOT NULL REFERENCES "user"(id) ON DELETE CASCADE,
    invited_by BIGINT REFERENCES "user"(id) ON DELETE SET NULL,
    role TEXT NOT NULL DEFAULT 'member',
    message TEXT NOT NULL DEFAULT '',
    created_at BIGINT NOT NULL DEFAULT (EXTRACT(EPOCH FROM NOW())::BIGINT),
    PRIMARY KEY (organization_id, invited_user_id),
    FOREIGN KEY (organization_id, role)
        REFERENCES organization_role(organization_id, role)
        ON UPDATE CASCADE ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_org_invite_user ON organization_invitation(invited_user_id);

CREATE TABLE IF NOT EXISTS user_inbox_message (
    id BIGSERIAL PRIMARY KEY,
    recipient_user_id BIGINT NOT NULL REFERENCES "user"(id) ON DELETE CASCADE,
    sender_user_id BIGINT REFERENCES "user"(id) ON DELETE SET NULL,
    subject TEXT NOT NULL,
    body TEXT NOT NULL,
    is_read BIGINT NOT NULL DEFAULT 0,
    created_at BIGINT NOT NULL DEFAULT (EXTRACT(EPOCH FROM NOW())::BIGINT)
);

CREATE INDEX IF NOT EXISTS idx_inbox_recipient ON user_inbox_message(recipient_user_id, is_read, created_at DESC);

CREATE TABLE IF NOT EXISTS task (
    id BIGSERIAL PRIMARY KEY,
    project_id BIGINT NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    author BIGINT REFERENCES "user"(id) ON DELETE SET NULL,
    status TEXT NOT NULL DEFAULT 'suggested'
        CHECK (status IN ('suggested', 'accepted', 'assigned', 'in_progress', 'finished')),
    loro_snapshot BYTEA
);

CREATE INDEX IF NOT EXISTS idx_task_project ON task(project_id, id);

CREATE TABLE IF NOT EXISTS assignments (
    task_id BIGINT NOT NULL REFERENCES task(id) ON DELETE CASCADE,
    assigned_to BIGINT NOT NULL REFERENCES "user"(id) ON DELETE CASCADE,
    PRIMARY KEY (task_id, assigned_to)
);

CREATE INDEX IF NOT EXISTS idx_assignments_user ON assignments(assigned_to, task_id);

CREATE TABLE IF NOT EXISTS task_message (
    task_id BIGINT NOT NULL REFERENCES task(id) ON DELETE CASCADE,
    msg_id BIGINT NOT NULL,
    author_id BIGINT REFERENCES "user"(id) ON DELETE SET NULL,
    created_at BIGINT NOT NULL DEFAULT (EXTRACT(EPOCH FROM NOW())::BIGINT),
    msg TEXT NOT NULL,
    PRIMARY KEY (task_id, msg_id)
);

