-- 0007_workflow_contract_drop_legacy_status.sql
-- Milestone M3: Contract Phase
-- Drops the legacy task.status column, enforces workflow_status_id NOT NULL,
-- updates triggers to inspect workflow_status_id directly, and adds workflow templates support.

-- 1. Create table for organization-defined and starter workflow templates
CREATE TABLE IF NOT EXISTS organization_workflow_template (
    id BIGSERIAL PRIMARY KEY,
    organization_id BIGINT REFERENCES organization(id) ON DELETE CASCADE,
    name VARCHAR(100) NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    statuses_json JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (organization_id, name)
);

-- 2. Drop the dual-write synchronization trigger from M1
DROP TRIGGER IF EXISTS trg_01_sync_task_status_and_workflow ON task;
DROP FUNCTION IF EXISTS fn_sync_task_status_and_workflow();

-- 3. Default newly inserted tasks without an explicit workflow_status_id to the project's initial status
CREATE OR REPLACE FUNCTION fn_default_task_workflow_status()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.workflow_status_id IS NULL AND NEW.project_id IS NOT NULL THEN
        SELECT id INTO NEW.workflow_status_id
        FROM project_workflow_status
        WHERE project_id = NEW.project_id AND is_initial = TRUE
        LIMIT 1;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_00_default_task_workflow_status ON task;
CREATE TRIGGER trg_00_default_task_workflow_status
BEFORE INSERT ON task
FOR EACH ROW EXECUTE FUNCTION fn_default_task_workflow_status();

-- 4. Rewrite Trigger 3.1: Task Assignment State & Permission Check
-- Strictly inspects project_workflow_status via task.workflow_status_id
CREATE OR REPLACE FUNCTION fn_check_assignment_insert()
RETURNS TRIGGER AS $$
DECLARE
    v_project_id BIGINT;
    v_workflow_status_id BIGINT;
    v_allows_assignment BOOLEAN;
BEGIN
    SELECT project_id, workflow_status_id
    INTO v_project_id, v_workflow_status_id
    FROM task WHERE id = NEW.task_id;

    SELECT allows_assignment INTO v_allows_assignment
    FROM project_workflow_status
    WHERE id = v_workflow_status_id;

    IF v_allows_assignment IS FALSE THEN
        RAISE EXCEPTION 'Cannot assign a task that has a status disallowing assignment';
    END IF;

    IF v_project_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM project_members
        WHERE project_id = v_project_id
          AND user_id = NEW.assigned_to
          AND has_perm(permissions_mask, 4)
    ) THEN
        RAISE EXCEPTION 'Cannot assign task to a user who lacks task execution permissions.';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- 5. Rewrite Trigger 3.2: Tasks requiring assignee must have at least one assignee
CREATE OR REPLACE FUNCTION fn_enforce_in_progress_assigned()
RETURNS TRIGGER AS $$
DECLARE
    v_requires_assignee BOOLEAN;
BEGIN
    SELECT requires_assignee INTO v_requires_assignee
    FROM project_workflow_status
    WHERE id = NEW.workflow_status_id;

    IF v_requires_assignee IS TRUE
       AND NOT EXISTS (SELECT 1 FROM assignments WHERE task_id = NEW.id) THEN
        RAISE EXCEPTION 'Cannot move task to this status without an assignee';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_02_enforce_in_progress_assigned ON task;
CREATE TRIGGER trg_02_enforce_in_progress_assigned
BEFORE INSERT OR UPDATE OF workflow_status_id ON task
FOR EACH ROW EXECUTE FUNCTION fn_enforce_in_progress_assigned();

-- 6. Rewrite Trigger 3.3: Prevent Removing the Last Assignee from a task requiring assignee
CREATE OR REPLACE FUNCTION fn_prevent_unassign_in_progress_task()
RETURNS TRIGGER AS $$
DECLARE
    v_workflow_status_id BIGINT;
    v_requires_assignee BOOLEAN;
BEGIN
    SELECT workflow_status_id
    INTO v_workflow_status_id
    FROM task WHERE id = OLD.task_id;

    SELECT requires_assignee INTO v_requires_assignee
    FROM project_workflow_status
    WHERE id = v_workflow_status_id;

    IF v_requires_assignee IS TRUE
       AND (SELECT COUNT(*) FROM assignments WHERE task_id = OLD.task_id) <= 1 THEN
        RAISE EXCEPTION 'Cannot remove the last assignee from a task requiring an assignee';
    END IF;

    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

-- 7. Rewrite user departure cleanup to unassign safely without reference to legacy status column
CREATE OR REPLACE FUNCTION fn_cleanup_user_project_assignments(p_project_id BIGINT, p_user_id BIGINT)
RETURNS VOID AS $$
DECLARE
    v_fallback_status_id BIGINT;
BEGIN
    -- Locate a status in the project that allows assignment but does not require an assignee
    SELECT id INTO v_fallback_status_id
    FROM project_workflow_status
    WHERE project_id = p_project_id
      AND requires_assignee = FALSE
      AND allows_assignment = TRUE
    ORDER BY position ASC
    LIMIT 1;

    -- If none found, fallback to initial landing status
    IF v_fallback_status_id IS NULL THEN
        SELECT id INTO v_fallback_status_id
        FROM project_workflow_status
        WHERE project_id = p_project_id AND is_initial = TRUE
        LIMIT 1;
    END IF;

    IF v_fallback_status_id IS NOT NULL THEN
        UPDATE task
        SET workflow_status_id = v_fallback_status_id
        WHERE project_id = p_project_id
          AND workflow_status_id IN (
              SELECT id FROM project_workflow_status
              WHERE project_id = p_project_id AND requires_assignee = TRUE
          )
          AND (SELECT COUNT(*) FROM assignments WHERE task_id = task.id) <= 1
          AND EXISTS (SELECT 1 FROM assignments WHERE task_id = task.id AND assigned_to = p_user_id);
    END IF;

    DELETE FROM assignments
    WHERE assigned_to = p_user_id
      AND task_id IN (SELECT id FROM task WHERE project_id = p_project_id);
END;
$$ LANGUAGE plpgsql;

-- 8. Backfill any task missing workflow_status_id with the project's initial status
UPDATE task t
SET workflow_status_id = (
    SELECT pws.id
    FROM project_workflow_status pws
    WHERE pws.project_id = t.project_id AND pws.is_initial = TRUE
    LIMIT 1
)
WHERE t.workflow_status_id IS NULL;

-- 9. Enforce NOT NULL on workflow_status_id
ALTER TABLE task ALTER COLUMN workflow_status_id SET NOT NULL;

-- 10. Drop the legacy status column and its check constraint from task
ALTER TABLE task DROP COLUMN IF EXISTS status;

