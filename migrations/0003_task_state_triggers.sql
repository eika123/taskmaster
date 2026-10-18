-- 0003_task_state_triggers.sql
-- PL/pgSQL triggers for Task State Machine and Access Control

-- 3.1 Task Assignment State & Permission Check
CREATE OR REPLACE FUNCTION fn_check_assignment_insert()
RETURNS TRIGGER AS $$
DECLARE
    v_status TEXT;
    v_project_id BIGINT;
BEGIN
    SELECT status, project_id INTO v_status, v_project_id
    FROM task WHERE id = NEW.task_id;

    IF v_status = 'suggested' THEN
        RAISE EXCEPTION 'Cannot assign a task that has suggested status';
    END IF;

    IF v_project_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM project_members
        WHERE project_id = v_project_id
          AND user_id = NEW.assigned_to
          AND (permissions_mask & 4) != 0
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

-- 3.2 In-Progress Tasks Require an Assignee
CREATE OR REPLACE FUNCTION fn_enforce_in_progress_assigned()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.status = 'in_progress'
       AND NOT EXISTS (SELECT 1 FROM assignments WHERE task_id = NEW.id) THEN
        RAISE EXCEPTION 'Cannot set status to in_progress without an assignee';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS enforce_in_progress_assigned ON task;
CREATE TRIGGER enforce_in_progress_assigned
BEFORE UPDATE OF status ON task
FOR EACH ROW EXECUTE FUNCTION fn_enforce_in_progress_assigned();

-- 3.3 Prevent Removing the Last Assignee from In-Progress Task
CREATE OR REPLACE FUNCTION fn_prevent_unassign_in_progress_task()
RETURNS TRIGGER AS $$
BEGIN
    IF EXISTS (SELECT 1 FROM task WHERE id = OLD.task_id AND status = 'in_progress')
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

-- 3.4 Task Author Mutation Permission Check
CREATE OR REPLACE FUNCTION fn_check_task_author_permission()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.project_id IS NOT NULL
       AND NEW.author IS NOT NULL
       AND NOT EXISTS (
           SELECT 1 FROM project_members
           WHERE project_id = NEW.project_id
             AND user_id = NEW.author
             AND (permissions_mask & 4) != 0
       ) THEN
        RAISE EXCEPTION 'User does not have permission to create or edit tasks in this project.';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS check_task_author_permission ON task;
CREATE TRIGGER check_task_author_permission
BEFORE INSERT ON task
FOR EACH ROW EXECUTE FUNCTION fn_check_task_author_permission();

-- 3.5 Task Discussion Message Author Permission Check
CREATE OR REPLACE FUNCTION fn_check_task_message_permission()
RETURNS TRIGGER AS $$
DECLARE
    v_project_id BIGINT;
BEGIN
    SELECT project_id INTO v_project_id FROM task WHERE id = NEW.task_id;
    IF v_project_id IS NOT NULL
       AND NEW.author_id IS NOT NULL
       AND NOT EXISTS (
           SELECT 1 FROM project_members
           WHERE project_id = v_project_id
             AND user_id = NEW.author_id
             AND (permissions_mask & 2) != 0
       ) THEN
        RAISE EXCEPTION 'User does not have permission to post messages in this project.';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS check_task_message_permission ON task_message;
CREATE TRIGGER check_task_message_permission
BEFORE INSERT ON task_message
FOR EACH ROW EXECUTE FUNCTION fn_check_task_message_permission();

