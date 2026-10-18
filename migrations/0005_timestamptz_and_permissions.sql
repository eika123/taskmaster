-- 0005_timestamptz_and_permissions.sql
-- Milestone M0: System-wide TIMESTAMPTZ unification, strongly-typed Permission SQL helpers,
-- and organization_role permissions mask.

-- 1. Helper function for bitwise permission checks
CREATE OR REPLACE FUNCTION has_perm(mask BIGINT, required BIGINT)
RETURNS BOOLEAN AS $$
BEGIN
    RETURN COALESCE((mask & required) != 0, FALSE);
END;
$$ LANGUAGE plpgsql IMMUTABLE;

-- 2. Add permissions_mask to organization_role
ALTER TABLE organization_role
    ADD COLUMN IF NOT EXISTS permissions_mask BIGINT NOT NULL DEFAULT 0;

-- Backfill existing organization roles with default permissions:
-- 'admin': 1023 (READ | SUGGEST | EXECUTE | ASSIGN | MANAGE_MEMBERS | ADMIN | MANAGE_WORKFLOW | MODERATE_DISCUSSIONS | POST_ANNOUNCEMENTS | MESSAGE_MEMBERS)
-- 'member': 1551 (READ | SUGGEST | EXECUTE | ASSIGN | MESSAGE_MEMBERS | IGNORABLE_BY_MEMBERS)
UPDATE organization_role
SET permissions_mask = 1023
WHERE role = 'admin' AND permissions_mask = 0;

UPDATE organization_role
SET permissions_mask = 1551
WHERE role = 'member' AND permissions_mask = 0;

-- Update trigger fn_seed_default_org_roles to assign default permissions masks
CREATE OR REPLACE FUNCTION fn_seed_default_org_roles()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO organization_role (organization_id, role, permissions_mask)
    VALUES (NEW.id, 'member', 1551), (NEW.id, 'admin', 1023)
    ON CONFLICT (organization_id, role) DO UPDATE
    SET permissions_mask = EXCLUDED.permissions_mask
    WHERE organization_role.permissions_mask = 0;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- 3. System-Wide TIMESTAMPTZ Migration: Convert all BIGINT epoch columns to TIMESTAMPTZ

-- 3.1 session
ALTER TABLE session
    ALTER COLUMN created_at DROP DEFAULT,
    ALTER COLUMN created_at TYPE TIMESTAMPTZ USING to_timestamp(created_at),
    ALTER COLUMN created_at SET DEFAULT NOW(),
    ALTER COLUMN expires_at DROP DEFAULT,
    ALTER COLUMN expires_at TYPE TIMESTAMPTZ USING to_timestamp(expires_at);

-- 3.2 organization
ALTER TABLE organization
    ALTER COLUMN created_at DROP DEFAULT,
    ALTER COLUMN created_at TYPE TIMESTAMPTZ USING to_timestamp(created_at),
    ALTER COLUMN created_at SET DEFAULT NOW();

-- 3.3 organization_members
ALTER TABLE organization_members
    ALTER COLUMN joined_at DROP DEFAULT,
    ALTER COLUMN joined_at TYPE TIMESTAMPTZ USING to_timestamp(joined_at),
    ALTER COLUMN joined_at SET DEFAULT NOW();

-- 3.4 organization_member_roles
ALTER TABLE organization_member_roles
    ALTER COLUMN assigned_at DROP DEFAULT,
    ALTER COLUMN assigned_at TYPE TIMESTAMPTZ USING to_timestamp(assigned_at),
    ALTER COLUMN assigned_at SET DEFAULT NOW();

-- 3.5 organization_governance_transfer
ALTER TABLE organization_governance_transfer
    ALTER COLUMN created_at DROP DEFAULT,
    ALTER COLUMN created_at TYPE TIMESTAMPTZ USING to_timestamp(created_at),
    ALTER COLUMN created_at SET DEFAULT NOW();

-- 3.6 project
ALTER TABLE project
    ALTER COLUMN created_at DROP DEFAULT,
    ALTER COLUMN created_at TYPE TIMESTAMPTZ USING to_timestamp(created_at),
    ALTER COLUMN created_at SET DEFAULT NOW();

-- 3.7 project_organizations
ALTER TABLE project_organizations
    ALTER COLUMN joined_at DROP DEFAULT,
    ALTER COLUMN joined_at TYPE TIMESTAMPTZ USING to_timestamp(joined_at),
    ALTER COLUMN joined_at SET DEFAULT NOW();

-- 3.8 project_members
ALTER TABLE project_members
    ALTER COLUMN joined_at DROP DEFAULT,
    ALTER COLUMN joined_at TYPE TIMESTAMPTZ USING to_timestamp(joined_at),
    ALTER COLUMN joined_at SET DEFAULT NOW();

-- 3.9 project_member_grants
ALTER TABLE project_member_grants
    ALTER COLUMN granted_at DROP DEFAULT,
    ALTER COLUMN granted_at TYPE TIMESTAMPTZ USING to_timestamp(granted_at),
    ALTER COLUMN granted_at SET DEFAULT NOW();

-- 3.10 project_leadership_transfer
ALTER TABLE project_leadership_transfer
    ALTER COLUMN created_at DROP DEFAULT,
    ALTER COLUMN created_at TYPE TIMESTAMPTZ USING to_timestamp(created_at),
    ALTER COLUMN created_at SET DEFAULT NOW();

-- 3.11 project_invitation
ALTER TABLE project_invitation
    ALTER COLUMN created_at DROP DEFAULT,
    ALTER COLUMN created_at TYPE TIMESTAMPTZ USING to_timestamp(created_at),
    ALTER COLUMN created_at SET DEFAULT NOW();

-- 3.12 organization_invitation
ALTER TABLE organization_invitation
    ALTER COLUMN created_at DROP DEFAULT,
    ALTER COLUMN created_at TYPE TIMESTAMPTZ USING to_timestamp(created_at),
    ALTER COLUMN created_at SET DEFAULT NOW();

-- 3.13 user_inbox_message
ALTER TABLE user_inbox_message
    ALTER COLUMN created_at DROP DEFAULT,
    ALTER COLUMN created_at TYPE TIMESTAMPTZ USING to_timestamp(created_at),
    ALTER COLUMN created_at SET DEFAULT NOW();

-- 3.14 task_message
ALTER TABLE task_message
    ALTER COLUMN created_at DROP DEFAULT,
    ALTER COLUMN created_at TYPE TIMESTAMPTZ USING to_timestamp(created_at),
    ALTER COLUMN created_at SET DEFAULT NOW();

-- 4. Update trigger functions to use NOW() instead of EXTRACT(EPOCH FROM NOW())::BIGINT
-- and use has_perm(...) for permission bit checks

CREATE OR REPLACE FUNCTION recompute_project_member(p_project_id BIGINT, p_user_id BIGINT)
RETURNS VOID AS $$
DECLARE
    v_union_mask BIGINT;
    v_pinned_mask BIGINT;
    v_effective_mask BIGINT;
    v_has_direct BOOLEAN;
    v_active_key TEXT;
    v_role TEXT;
BEGIN
    SELECT
        BIT_OR(prd.permissions_mask),
        BOOL_OR(pmg.grant_key = 'direct')
    INTO v_union_mask, v_has_direct
    FROM project_member_grants pmg
    JOIN project_role_definition prd ON prd.role = pmg.project_role
    WHERE pmg.project_id = p_project_id AND pmg.user_id = p_user_id;

    IF v_union_mask IS NULL THEN
        DELETE FROM project_members
        WHERE project_id = p_project_id AND user_id = p_user_id;
        RETURN;
    END IF;

    SELECT active_grant_key INTO v_active_key
    FROM project_members
    WHERE project_id = p_project_id AND user_id = p_user_id;

    IF v_active_key IS NOT NULL THEN
        SELECT prd.permissions_mask INTO v_pinned_mask
        FROM project_member_grants pmg
        JOIN project_role_definition prd ON prd.role = pmg.project_role
        WHERE pmg.project_id = p_project_id
          AND pmg.user_id = p_user_id
          AND pmg.grant_key = v_active_key;

        IF v_pinned_mask IS NULL THEN
            v_active_key := NULL;
        END IF;
    END IF;

    v_effective_mask := COALESCE(v_pinned_mask, v_union_mask);
    v_role := CASE
        WHEN has_perm(v_effective_mask, 32) THEN 'admin'
        WHEN has_perm(v_effective_mask, 4) THEN 'member'
        ELSE 'viewer'
    END;

    INSERT INTO project_members (
        project_id, user_id, role, permissions_mask, granted_by_org_role, active_grant_key, joined_at
    )
    VALUES (
        p_project_id,
        p_user_id,
        v_role,
        v_effective_mask,
        CASE WHEN v_has_direct THEN 0 ELSE 1 END,
        v_active_key,
        NOW()
    )
    ON CONFLICT (project_id, user_id) DO UPDATE SET
        role = EXCLUDED.role,
        permissions_mask = EXCLUDED.permissions_mask,
        granted_by_org_role = EXCLUDED.granted_by_org_role,
        active_grant_key = EXCLUDED.active_grant_key;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION fn_sync_org_member_roles()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        INSERT INTO organization_member_roles (organization_id, user_id, role, assigned_at)
        VALUES (NEW.organization_id, NEW.user_id, NEW.role, NEW.joined_at)
        ON CONFLICT DO NOTHING;
    ELSIF TG_OP = 'UPDATE' AND OLD.role IS DISTINCT FROM NEW.role THEN
        DELETE FROM organization_member_roles
        WHERE organization_id = OLD.organization_id
          AND user_id = OLD.user_id
          AND role = OLD.role;

        INSERT INTO organization_member_roles (organization_id, user_id, role, assigned_at)
        VALUES (NEW.organization_id, NEW.user_id, NEW.role, NOW())
        ON CONFLICT DO NOTHING;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION fn_sync_grants_on_rule_change()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        INSERT INTO project_member_grants (
            project_id, user_id, grant_key, project_role, source_org_id, source_org_role, granted_at
        )
        SELECT
            NEW.project_id,
            omr.user_id,
            'org:' || NEW.organization_id || ':' || NEW.org_role,
            NEW.project_role,
            NEW.organization_id,
            NEW.org_role,
            NOW()
        FROM organization_member_roles omr
        WHERE omr.organization_id = NEW.organization_id
          AND omr.role = NEW.org_role
        ON CONFLICT (project_id, user_id, grant_key) DO UPDATE SET
            project_role = EXCLUDED.project_role,
            granted_at = EXCLUDED.granted_at;
    ELSIF TG_OP = 'UPDATE' AND OLD.project_role IS DISTINCT FROM NEW.project_role THEN
        UPDATE project_member_grants
        SET project_role = NEW.project_role
        WHERE project_id = NEW.project_id
          AND source_org_id = NEW.organization_id
          AND source_org_role = NEW.org_role;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION fn_sync_grants_after_org_role_insert()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO project_member_grants (
        project_id, user_id, grant_key, project_role, source_org_id, source_org_role, granted_at
    )
    SELECT
        pora.project_id,
        NEW.user_id,
        'org:' || NEW.organization_id || ':' || NEW.role,
        pora.project_role,
        NEW.organization_id,
        NEW.role,
        NOW()
    FROM project_org_role_access pora
    WHERE pora.organization_id = NEW.organization_id
      AND pora.org_role = NEW.role
    ON CONFLICT (project_id, user_id, grant_key) DO UPDATE SET
        project_role = EXCLUDED.project_role,
        granted_at = EXCLUDED.granted_at;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION fn_cleanup_assignments_on_revoke_execute()
RETURNS TRIGGER AS $$
BEGIN
    IF NOT has_perm(NEW.permissions_mask, 4) AND has_perm(OLD.permissions_mask, 4) THEN
        DELETE FROM assignments
        WHERE assigned_to = NEW.user_id
          AND task_id IN (
              SELECT id FROM task WHERE project_id = NEW.project_id
          );
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

