-- 0006_workflow_engine_expand.sql
-- Milestone M1: Dynamic Workflow Statuses, Transitions, Semantic Flags, and Composite Task Foreign Keys.

-- 1. Create project_workflow_status table
CREATE TABLE IF NOT EXISTS project_workflow_status (
    id BIGSERIAL PRIMARY KEY,
    project_id BIGINT NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    name VARCHAR(60) NOT NULL,
    category TEXT NOT NULL DEFAULT 'todo'
        CHECK (category IN ('todo', 'in_progress', 'review', 'done', 'archived')),
    badge_variant TEXT NOT NULL DEFAULT 'default'
        CHECK (badge_variant IN ('default', 'info', 'success', 'warning', 'danger', 'purple', 'teal')),
    position BIGINT NOT NULL,
    wip_limit INT NULL CHECK (wip_limit IS NULL OR wip_limit > 0),
    is_initial BOOLEAN NOT NULL DEFAULT FALSE,
    allows_assignment BOOLEAN NOT NULL DEFAULT TRUE,
    requires_assignee BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (project_id, id),
    UNIQUE (project_id, name)
);

-- 2. Exactly one initial landing status per project
CREATE UNIQUE INDEX IF NOT EXISTS idx_one_initial_status_per_project
    ON project_workflow_status(project_id)
    WHERE is_initial = TRUE;

-- 3. Create project_workflow_transition table
CREATE TABLE IF NOT EXISTS project_workflow_transition (
    project_id BIGINT NOT NULL,
    from_status_id BIGINT NOT NULL,
    to_status_id BIGINT NOT NULL,
    required_permission BIGINT NOT NULL DEFAULT 4, -- Permission::EXECUTE
    PRIMARY KEY (project_id, from_status_id, to_status_id),
    FOREIGN KEY (project_id, from_status_id)
        REFERENCES project_workflow_status(project_id, id) ON DELETE CASCADE,
    FOREIGN KEY (project_id, to_status_id)
        REFERENCES project_workflow_status(project_id, id) ON DELETE CASCADE
);

-- 4. Backfill all existing projects with the default 5-state "Legacy Flow"
INSERT INTO project_workflow_status (
    project_id, name, category, badge_variant, position, wip_limit, is_initial, allows_assignment, requires_assignee
)
SELECT
    p.id, s.name, s.category, s.badge_variant, s.position, s.wip_limit, s.is_initial, s.allows_assignment, s.requires_assignee
FROM project p
CROSS JOIN (
    VALUES
        ('Suggested',  'todo',        'default', 1000::bigint, NULL::int, TRUE,  FALSE, FALSE),
        ('Accepted',   'todo',        'info',    2000::bigint, NULL::int, FALSE, TRUE,  FALSE),
        ('Assigned',   'todo',        'purple',  3000::bigint, NULL::int, FALSE, TRUE,  FALSE),
        ('InProgress', 'in_progress', 'warning', 4000::bigint, NULL::int, FALSE, TRUE,  TRUE),
        ('Finished',   'done',        'success', 5000::bigint, NULL::int, FALSE, TRUE,  FALSE)
) AS s(name, category, badge_variant, position, wip_limit, is_initial, allows_assignment, requires_assignee)
ON CONFLICT (project_id, name) DO NOTHING;

-- 5. Trigger to automatically seed newly created projects with default workflow statuses
CREATE OR REPLACE FUNCTION fn_seed_default_project_workflow()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO project_workflow_status (
        project_id, name, category, badge_variant, position, wip_limit, is_initial, allows_assignment, requires_assignee
    ) VALUES
        (NEW.id, 'Suggested',  'todo',        'default', 1000, NULL, TRUE,  FALSE, FALSE),
        (NEW.id, 'Accepted',   'todo',        'info',    2000, NULL, FALSE, TRUE,  FALSE),
        (NEW.id, 'Assigned',   'todo',        'purple',  3000, NULL, FALSE, TRUE,  FALSE),
        (NEW.id, 'InProgress', 'in_progress', 'warning', 4000, NULL, FALSE, TRUE,  TRUE),
        (NEW.id, 'Finished',   'done',        'success', 5000, NULL, FALSE, TRUE,  FALSE)
    ON CONFLICT (project_id, name) DO NOTHING;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_seed_default_project_workflow ON project;
CREATE TRIGGER trg_seed_default_project_workflow
AFTER INSERT ON project
FOR EACH ROW EXECUTE FUNCTION fn_seed_default_project_workflow();

-- 6. Add composite key constraint and workflow columns to task
ALTER TABLE task
    ADD CONSTRAINT uq_task_project_id_id UNIQUE (project_id, id),
    ADD COLUMN IF NOT EXISTS workflow_status_id BIGINT,
    ADD COLUMN IF NOT EXISTS start_date TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS due_date TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS estimated_hours INT CHECK (estimated_hours IS NULL OR estimated_hours >= 0),
    ADD COLUMN IF NOT EXISTS progress_percent INT NOT NULL DEFAULT 0 CHECK (progress_percent BETWEEN 0 AND 100),
    ADD CONSTRAINT chk_task_dates CHECK (due_date IS NULL OR start_date IS NULL OR due_date >= start_date);

-- 7. Backfill task.workflow_status_id for existing tasks by matching project_id and status
UPDATE task t
SET workflow_status_id = pws.id
FROM project_workflow_status pws
WHERE pws.project_id = t.project_id
  AND (
      LOWER(pws.name) = LOWER(t.status)
      OR (t.status = 'in_progress' AND LOWER(pws.name) IN ('inprogress', 'in_progress'))
  );

-- 8. Add foreign key from task(project_id, workflow_status_id) to project_workflow_status(project_id, id)
ALTER TABLE task
    ADD CONSTRAINT fk_task_workflow_status
    FOREIGN KEY (project_id, workflow_status_id)
    REFERENCES project_workflow_status(project_id, id)
    ON DELETE RESTRICT;

-- 9. Trigger to keep task.status and task.workflow_status_id in sync during the dual-write Expand phase
CREATE OR REPLACE FUNCTION fn_sync_task_status_and_workflow()
RETURNS TRIGGER AS $$
DECLARE
    v_status_name TEXT;
    v_ws_id BIGINT;
BEGIN
    -- If workflow_status_id was provided or changed, sync legacy status
    IF NEW.workflow_status_id IS NOT NULL AND (TG_OP = 'INSERT' OR NEW.workflow_status_id IS DISTINCT FROM OLD.workflow_status_id) THEN
        SELECT LOWER(name) INTO v_status_name
        FROM project_workflow_status
        WHERE id = NEW.workflow_status_id AND project_id = NEW.project_id;

        IF v_status_name IS NOT NULL THEN
            IF v_status_name IN ('inprogress', 'in_progress') THEN
                NEW.status := 'in_progress';
            ELSIF v_status_name IN ('suggested', 'accepted', 'assigned', 'finished') THEN
                NEW.status := v_status_name;
            END IF;
        END IF;
    -- If status was provided but workflow_status_id is NULL, or status changed, sync workflow_status_id
    ELSIF NEW.project_id IS NOT NULL AND (NEW.workflow_status_id IS NULL OR (TG_OP = 'UPDATE' AND NEW.status IS DISTINCT FROM OLD.status)) THEN
        SELECT id INTO v_ws_id
        FROM project_workflow_status
        WHERE project_id = NEW.project_id
          AND (
              LOWER(name) = LOWER(NEW.status)
              OR (NEW.status = 'in_progress' AND LOWER(name) IN ('inprogress', 'in_progress'))
          );
        IF v_ws_id IS NOT NULL THEN
            NEW.workflow_status_id := v_ws_id;
        END IF;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_01_sync_task_status_and_workflow ON task;
CREATE TRIGGER trg_01_sync_task_status_and_workflow
BEFORE INSERT OR UPDATE OF status, workflow_status_id ON task
FOR EACH ROW EXECUTE FUNCTION fn_sync_task_status_and_workflow();

-- 10. Rewrite Triggers 3.1, 3.2, 3.3 to check dynamic semantic flags on project_workflow_status

-- 10.1 Trigger 3.1: Task Assignment State & Permission Check
CREATE OR REPLACE FUNCTION fn_check_assignment_insert()
RETURNS TRIGGER AS $$
DECLARE
    v_status TEXT;
    v_project_id BIGINT;
    v_workflow_status_id BIGINT;
    v_allows_assignment BOOLEAN;
BEGIN
    SELECT status, project_id, workflow_status_id
    INTO v_status, v_project_id, v_workflow_status_id
    FROM task WHERE id = NEW.task_id;

    IF v_workflow_status_id IS NOT NULL THEN
        SELECT allows_assignment INTO v_allows_assignment
        FROM project_workflow_status
        WHERE id = v_workflow_status_id;
    ELSIF v_project_id IS NOT NULL THEN
        SELECT allows_assignment INTO v_allows_assignment
        FROM project_workflow_status
        WHERE project_id = v_project_id
          AND (
              LOWER(name) = LOWER(v_status)
              OR (v_status = 'in_progress' AND LOWER(name) IN ('inprogress', 'in_progress'))
          );
    END IF;

    IF v_allows_assignment IS FALSE OR (v_allows_assignment IS NULL AND v_status = 'suggested') THEN
        RAISE EXCEPTION 'Cannot assign a task that has suggested status';
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

DROP TRIGGER IF EXISTS check_assignment_insert ON assignments;
CREATE TRIGGER check_assignment_insert
BEFORE INSERT ON assignments
FOR EACH ROW EXECUTE FUNCTION fn_check_assignment_insert();

-- 10.2 Trigger 3.2: Tasks requiring assignee must have at least one assignee
CREATE OR REPLACE FUNCTION fn_enforce_in_progress_assigned()
RETURNS TRIGGER AS $$
DECLARE
    v_requires_assignee BOOLEAN;
BEGIN
    IF NEW.workflow_status_id IS NOT NULL THEN
        SELECT requires_assignee INTO v_requires_assignee
        FROM project_workflow_status
        WHERE id = NEW.workflow_status_id;
    ELSIF NEW.project_id IS NOT NULL THEN
        SELECT requires_assignee INTO v_requires_assignee
        FROM project_workflow_status
        WHERE project_id = NEW.project_id
          AND (
              LOWER(name) = LOWER(NEW.status)
              OR (NEW.status = 'in_progress' AND LOWER(name) IN ('inprogress', 'in_progress'))
          );
    END IF;

    IF (v_requires_assignee IS TRUE OR (v_requires_assignee IS NULL AND NEW.status = 'in_progress'))
       AND NOT EXISTS (SELECT 1 FROM assignments WHERE task_id = NEW.id) THEN
        RAISE EXCEPTION 'Cannot set status to in_progress without an assignee';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS enforce_in_progress_assigned ON task;
DROP TRIGGER IF EXISTS trg_02_enforce_in_progress_assigned ON task;
CREATE TRIGGER trg_02_enforce_in_progress_assigned
BEFORE INSERT OR UPDATE OF status, workflow_status_id ON task
FOR EACH ROW EXECUTE FUNCTION fn_enforce_in_progress_assigned();

-- 10.3 Trigger 3.3: Prevent Removing the Last Assignee from a task that requires an assignee
CREATE OR REPLACE FUNCTION fn_prevent_unassign_in_progress_task()
RETURNS TRIGGER AS $$
DECLARE
    v_status TEXT;
    v_project_id BIGINT;
    v_workflow_status_id BIGINT;
    v_requires_assignee BOOLEAN;
BEGIN
    SELECT status, project_id, workflow_status_id
    INTO v_status, v_project_id, v_workflow_status_id
    FROM task WHERE id = OLD.task_id;

    IF v_workflow_status_id IS NOT NULL THEN
        SELECT requires_assignee INTO v_requires_assignee
        FROM project_workflow_status
        WHERE id = v_workflow_status_id;
    ELSIF v_project_id IS NOT NULL THEN
        SELECT requires_assignee INTO v_requires_assignee
        FROM project_workflow_status
        WHERE project_id = v_project_id
          AND (
              LOWER(name) = LOWER(v_status)
              OR (v_status = 'in_progress' AND LOWER(name) IN ('inprogress', 'in_progress'))
          );
    END IF;

    IF (v_requires_assignee IS TRUE OR (v_requires_assignee IS NULL AND v_status = 'in_progress'))
       AND (SELECT COUNT(*) FROM assignments WHERE task_id = OLD.task_id) <= 1 THEN
        RAISE EXCEPTION 'Cannot remove the last assignee from an in_progress task';
    END IF;

    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS prevent_unassign_in_progress_task ON assignments;
CREATE TRIGGER prevent_unassign_in_progress_task
BEFORE DELETE ON assignments
FOR EACH ROW EXECUTE FUNCTION fn_prevent_unassign_in_progress_task();

