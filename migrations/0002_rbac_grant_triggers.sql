-- 0002_rbac_grant_triggers.sql
-- PL/pgSQL functions and triggers for Organization RBAC, Privilege Ceilings, and Grant Ledger Materialization

-- Helper: Recompute materialized project_members entry from project_member_grants
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
        WHEN (v_effective_mask & 32) != 0 THEN 'admin'
        WHEN (v_effective_mask & 4) != 0 THEN 'member'
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
        EXTRACT(EPOCH FROM NOW())::BIGINT
    )
    ON CONFLICT (project_id, user_id) DO UPDATE SET
        role = EXCLUDED.role,
        permissions_mask = EXCLUDED.permissions_mask,
        granted_by_org_role = EXCLUDED.granted_by_org_role,
        active_grant_key = EXCLUDED.active_grant_key;
END;
$$ LANGUAGE plpgsql;

-- 2.1 Seed default 'member' and 'admin' roles when an organization is created
CREATE OR REPLACE FUNCTION fn_seed_default_org_roles()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO organization_role (organization_id, role)
    VALUES (NEW.id, 'member'), (NEW.id, 'admin')
    ON CONFLICT DO NOTHING;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS seed_default_org_roles ON organization;
CREATE TRIGGER seed_default_org_roles
AFTER INSERT ON organization
FOR EACH ROW EXECUTE FUNCTION fn_seed_default_org_roles();

-- 2.2 Protect built-in ('member', 'admin') and actively assigned roles while organization exists
CREATE OR REPLACE FUNCTION fn_protect_builtin_org_roles()
RETURNS TRIGGER AS $$
BEGIN
    IF EXISTS (SELECT 1 FROM organization WHERE id = OLD.organization_id) THEN
        IF OLD.role IN ('member', 'admin') THEN
            RAISE EXCEPTION 'Cannot delete built-in organization roles (admin, member).';
        END IF;
        IF EXISTS (
            SELECT 1 FROM organization_members
            WHERE organization_id = OLD.organization_id AND role = OLD.role
        ) OR EXISTS (
            SELECT 1 FROM organization_member_roles
            WHERE organization_id = OLD.organization_id AND role = OLD.role
        ) THEN
            RAISE EXCEPTION 'Cannot delete organization role while it is assigned to members.';
        END IF;
    END IF;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS protect_builtin_org_roles ON organization_role;
CREATE TRIGGER protect_builtin_org_roles
BEFORE DELETE ON organization_role
FOR EACH ROW EXECUTE FUNCTION fn_protect_builtin_org_roles();

-- 2.3 Organization deletion guardrail
CREATE OR REPLACE FUNCTION fn_prevent_delete_populated_org()
RETURNS TRIGGER AS $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM organization_members
        WHERE organization_id = OLD.id AND user_id != OLD.governing_user_id
    ) THEN
        RAISE EXCEPTION 'Cannot delete an organization while other members remain. Remove all other members first.';
    END IF;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS prevent_delete_populated_org ON organization;
CREATE TRIGGER prevent_delete_populated_org
BEFORE DELETE ON organization
FOR EACH ROW EXECUTE FUNCTION fn_prevent_delete_populated_org();

-- 2.4 Governing user removal guardrail
CREATE OR REPLACE FUNCTION fn_prevent_remove_governing_user()
RETURNS TRIGGER AS $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM organization
        WHERE id = OLD.organization_id AND governing_user_id = OLD.user_id
    ) THEN
        RAISE EXCEPTION 'The governing user cannot be removed from the organization. Transfer governance first.';
    END IF;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS prevent_remove_governing_user ON organization_members;
CREATE TRIGGER prevent_remove_governing_user
BEFORE DELETE ON organization_members
FOR EACH ROW EXECUTE FUNCTION fn_prevent_remove_governing_user();

-- 2.5 Sync organization_member_roles when organization_members is inserted or updated
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
        VALUES (NEW.organization_id, NEW.user_id, NEW.role, EXTRACT(EPOCH FROM NOW())::BIGINT)
        ON CONFLICT DO NOTHING;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS sync_org_member_roles_after_member_insert ON organization_members;
CREATE TRIGGER sync_org_member_roles_after_member_insert
AFTER INSERT OR UPDATE OF role ON organization_members
FOR EACH ROW EXECUTE FUNCTION fn_sync_org_member_roles();

-- 2.6 Project deletion & leader removal guardrails
CREATE OR REPLACE FUNCTION fn_prevent_delete_populated_project()
RETURNS TRIGGER AS $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM project_members
        WHERE project_id = OLD.id AND user_id != OLD.leader_id
    ) THEN
        RAISE EXCEPTION 'Cannot delete a project while other members remain. Remove all other members first.';
    END IF;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS prevent_delete_populated_project ON project;
CREATE TRIGGER prevent_delete_populated_project
BEFORE DELETE ON project
FOR EACH ROW EXECUTE FUNCTION fn_prevent_delete_populated_project();

CREATE OR REPLACE FUNCTION fn_prevent_remove_project_leader()
RETURNS TRIGGER AS $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM project
        WHERE id = OLD.project_id AND leader_id = OLD.user_id
    ) THEN
        RAISE EXCEPTION 'The project leader cannot be removed from the project. Transfer project leadership first.';
    END IF;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS prevent_remove_project_leader ON project_members;
CREATE TRIGGER prevent_remove_project_leader
BEFORE DELETE ON project_members
FOR EACH ROW EXECUTE FUNCTION fn_prevent_remove_project_leader();

-- 2.7 Auto-affiliate owner organization when a project is created with owner_org_id
CREATE OR REPLACE FUNCTION fn_sync_project_owner_org_insert()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.owner_org_id IS NOT NULL THEN
        INSERT INTO project_organizations (project_id, organization_id, relationship_type, max_granted_role)
        VALUES (NEW.id, NEW.owner_org_id, 'owner', 'admin')
        ON CONFLICT DO NOTHING;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS sync_project_owner_org_insert ON project;
CREATE TRIGGER sync_project_owner_org_insert
AFTER INSERT ON project
FOR EACH ROW EXECUTE FUNCTION fn_sync_project_owner_org_insert();

-- 2.8 Privilege ceiling enforcement on project_org_role_access and project_organizations
CREATE OR REPLACE FUNCTION fn_check_org_project_role_ceiling()
RETURNS TRIGGER AS $$
DECLARE
    v_exceeds BOOLEAN;
BEGIN
    SELECT (prd_grant.permissions_mask & ~prd_ceiling.permissions_mask) != 0
    INTO v_exceeds
    FROM project_role_definition prd_grant,
         project_organizations po
         JOIN project_role_definition prd_ceiling ON prd_ceiling.role = po.max_granted_role
    WHERE prd_grant.role = NEW.project_role
      AND po.project_id = NEW.project_id
      AND po.organization_id = NEW.organization_id;

    IF COALESCE(v_exceeds, FALSE) THEN
        RAISE EXCEPTION 'Granted project role exceeds the organization privilege ceiling for this project.';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS check_org_project_role_ceiling ON project_org_role_access;
CREATE TRIGGER check_org_project_role_ceiling
BEFORE INSERT OR UPDATE OF project_role ON project_org_role_access
FOR EACH ROW EXECUTE FUNCTION fn_check_org_project_role_ceiling();

CREATE OR REPLACE FUNCTION fn_check_org_max_role_ceiling_update()
RETURNS TRIGGER AS $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM project_org_role_access pora
        JOIN project_role_definition prd_grant ON prd_grant.role = pora.project_role
        JOIN project_role_definition prd_ceiling ON prd_ceiling.role = NEW.max_granted_role
        WHERE pora.project_id = NEW.project_id
          AND pora.organization_id = NEW.organization_id
          AND (prd_grant.permissions_mask & ~prd_ceiling.permissions_mask) != 0
    ) THEN
        RAISE EXCEPTION 'Cannot lower organization privilege ceiling below existing role access rules. Update role rules first.';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS check_org_max_role_ceiling_update ON project_organizations;
CREATE TRIGGER check_org_max_role_ceiling_update
BEFORE UPDATE OF max_granted_role ON project_organizations
FOR EACH ROW EXECUTE FUNCTION fn_check_org_max_role_ceiling_update();

-- 2.9 Grant Ledger Synchronization from project_org_role_access & organization_member_roles
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
            EXTRACT(EPOCH FROM NOW())::BIGINT
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

DROP TRIGGER IF EXISTS sync_grants_on_rule_change ON project_org_role_access;
CREATE TRIGGER sync_grants_on_rule_change
AFTER INSERT OR UPDATE OF project_role ON project_org_role_access
FOR EACH ROW EXECUTE FUNCTION fn_sync_grants_on_rule_change();

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
        EXTRACT(EPOCH FROM NOW())::BIGINT
    FROM project_org_role_access pora
    WHERE pora.organization_id = NEW.organization_id
      AND pora.org_role = NEW.role
    ON CONFLICT (project_id, user_id, grant_key) DO UPDATE SET
        project_role = EXCLUDED.project_role,
        granted_at = EXCLUDED.granted_at;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS sync_grants_after_org_role_insert ON organization_member_roles;
CREATE TRIGGER sync_grants_after_org_role_insert
AFTER INSERT ON organization_member_roles
FOR EACH ROW EXECUTE FUNCTION fn_sync_grants_after_org_role_insert();

-- 2.10 Single Materialization Trigger on project_member_grants
CREATE OR REPLACE FUNCTION fn_sync_pm_from_grants()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        PERFORM recompute_project_member(OLD.project_id, OLD.user_id);
        RETURN OLD;
    ELSE
        PERFORM recompute_project_member(NEW.project_id, NEW.user_id);
        RETURN NEW;
    END IF;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS sync_pm_from_grants ON project_member_grants;
CREATE TRIGGER sync_pm_from_grants
AFTER INSERT OR UPDATE OR DELETE ON project_member_grants
FOR EACH ROW EXECUTE FUNCTION fn_sync_pm_from_grants();

-- 2.11 Direct project_members insert/delete & assignment cleanup triggers
CREATE OR REPLACE FUNCTION fn_sync_direct_grant_on_pm_insert()
RETURNS TRIGGER AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM project_member_grants
        WHERE project_id = NEW.project_id AND user_id = NEW.user_id
    ) THEN
        INSERT INTO project_member_grants (
            project_id, user_id, grant_key, project_role, source_org_id, source_org_role, granted_at
        ) VALUES (
            NEW.project_id, NEW.user_id, 'direct', NEW.role, NULL, NULL, NEW.joined_at
        )
        ON CONFLICT DO NOTHING;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS sync_direct_grant_on_pm_insert ON project_members;
CREATE TRIGGER sync_direct_grant_on_pm_insert
AFTER INSERT ON project_members
FOR EACH ROW EXECUTE FUNCTION fn_sync_direct_grant_on_pm_insert();

CREATE OR REPLACE FUNCTION fn_cleanup_user_project_assignments(p_project_id BIGINT, p_user_id BIGINT)
RETURNS VOID AS $$
BEGIN
    UPDATE task
    SET status = 'accepted'
    WHERE project_id = p_project_id
      AND status = 'in_progress'
      AND (SELECT COUNT(*) FROM assignments WHERE task_id = task.id) <= 1
      AND EXISTS (SELECT 1 FROM assignments WHERE task_id = task.id AND assigned_to = p_user_id);

    DELETE FROM assignments
    WHERE assigned_to = p_user_id
      AND task_id IN (SELECT id FROM task WHERE project_id = p_project_id);
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION fn_sync_grants_on_pm_delete()
RETURNS TRIGGER AS $$
BEGIN
    DELETE FROM project_member_grants
    WHERE project_id = OLD.project_id AND user_id = OLD.user_id;

    PERFORM fn_cleanup_user_project_assignments(OLD.project_id, OLD.user_id);
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS sync_grants_on_pm_delete ON project_members;
CREATE TRIGGER sync_grants_on_pm_delete
AFTER DELETE ON project_members
FOR EACH ROW EXECUTE FUNCTION fn_sync_grants_on_pm_delete();

CREATE OR REPLACE FUNCTION fn_cleanup_assignments_on_pm_demote()
RETURNS TRIGGER AS $$
BEGIN
    IF (NEW.permissions_mask & 4) = 0 AND (OLD.permissions_mask & 4) != 0 THEN
        PERFORM fn_cleanup_user_project_assignments(NEW.project_id, NEW.user_id);
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS cleanup_assignments_on_pm_demote ON project_members;
CREATE TRIGGER cleanup_assignments_on_pm_demote
AFTER UPDATE OF permissions_mask ON project_members
FOR EACH ROW EXECUTE FUNCTION fn_cleanup_assignments_on_pm_demote();

