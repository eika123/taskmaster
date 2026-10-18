use super::host::ProjectViewHost;
use crate::types::{
    BadgeVariant, BoardSnapshot, ProjectViewKind, StatusCategory, Task, TaskStatus, User,
    WorkflowStatusDto,
};
use chrono::Utc;
use dioxus::prelude::*;

#[derive(Clone, Routable, PartialEq)]
enum TestViewRoute {
    #[route("/")]
    TestViewPage {},
}

#[component]
fn TestViewPage() -> Element {
    let view = use_context::<Signal<ProjectViewKind>>();
    let board = use_context::<Signal<BoardSnapshot>>();
    rsx! {
        ProjectViewHost {
            view_kind: view,
            board: board,
            on_move_task: |_| {},
        }
    }
}

#[component]
fn TestViewWrapper(view: ProjectViewKind, board: BoardSnapshot) -> Element {
    let view_sig = use_signal(|| view);
    let board_sig = use_signal(|| board);
    use_context_provider(|| view_sig);
    use_context_provider(|| board_sig);

    rsx! {
        Router::<TestViewRoute> {}
    }
}

fn render_view_vdom(view: ProjectViewKind, board: BoardSnapshot) -> String {
    let mut vdom =
        VirtualDom::new_with_props(TestViewWrapper, TestViewWrapperProps { view, board });
    vdom.rebuild_in_place();
    dioxus::ssr::render(&vdom)
}

fn create_sample_status(
    id: i64,
    name: &str,
    category: StatusCategory,
    badge: BadgeVariant,
    wip_limit: Option<i32>,
    is_initial: bool,
) -> WorkflowStatusDto {
    WorkflowStatusDto {
        id,
        project_id: 1,
        name: name.to_string(),
        category,
        badge_variant: badge,
        position: id * 1000,
        wip_limit,
        is_initial,
        allows_assignment: true,
        requires_assignee: false,
        task_count: 0,
        wip_exceeded: false,
        created_at: Utc::now(),
    }
}

fn create_sample_task(
    id: i64,
    title: &str,
    status: TaskStatus,
    ws_id: Option<i64>,
    assignees: Vec<User>,
) -> Task {
    Task {
        id,
        project_id: 1,
        project_name: "Test Project".to_string(),
        title: title.to_string(),
        author: Some(1),
        author_name: "Lead Dev".to_string(),
        description: "Task description for testing".to_string(),
        status,
        workflow_status_id: ws_id,
        start_date: Some(Utc::now()),
        due_date: Some(Utc::now()),
        estimated_hours: Some(8),
        progress_percent: 25,
        assignees,
    }
}

// ============================================================================
// TC2 Fixtures
// ============================================================================

/// Fixture 1: Empty project
fn fixture_empty_project() -> BoardSnapshot {
    let statuses = vec![
        create_sample_status(
            1,
            "To Do",
            StatusCategory::Todo,
            BadgeVariant::Default,
            None,
            true,
        ),
        create_sample_status(
            2,
            "In Progress",
            StatusCategory::InProgress,
            BadgeVariant::Info,
            None,
            false,
        ),
        create_sample_status(
            3,
            "Done",
            StatusCategory::Done,
            BadgeVariant::Success,
            None,
            false,
        ),
    ];
    BoardSnapshot {
        project_id: 1,
        project_name: "Empty Project".to_string(),
        enabled_views: ProjectViewKind::all().to_vec(),
        default_view: ProjectViewKind::KanbanClassic,
        caller_permissions: 63,
        statuses,
        tasks: Vec::new(),
        dependencies: Vec::new(),
    }
}

/// Fixture 2: Single status
fn fixture_single_status() -> BoardSnapshot {
    let statuses = vec![create_sample_status(
        10,
        "Backlog",
        StatusCategory::Todo,
        BadgeVariant::Default,
        None,
        true,
    )
    .with_task_count(3)];
    let tasks = vec![
        create_sample_task(101, "Setup CI", TaskStatus::Suggested, Some(10), vec![]),
        create_sample_task(102, "Dockerize", TaskStatus::Suggested, Some(10), vec![]),
        create_sample_task(103, "Docs", TaskStatus::Suggested, Some(10), vec![]),
    ];
    BoardSnapshot {
        project_id: 2,
        project_name: "Single Status Project".to_string(),
        enabled_views: ProjectViewKind::all().to_vec(),
        default_view: ProjectViewKind::KanbanClassic,
        caller_permissions: 63,
        statuses,
        tasks,
        dependencies: Vec::new(),
    }
}

/// Fixture 3: 10 custom statuses with diverse badge variants
fn fixture_ten_custom_statuses() -> BoardSnapshot {
    let raw = [
        (1, "Triage", StatusCategory::Todo, BadgeVariant::Default),
        (2, "Design", StatusCategory::Todo, BadgeVariant::Purple),
        (3, "Farming", StatusCategory::InProgress, BadgeVariant::Teal),
        (
            4,
            "Development",
            StatusCategory::InProgress,
            BadgeVariant::Info,
        ),
        (
            5,
            "Code Review",
            StatusCategory::Review,
            BadgeVariant::Warning,
        ),
        (
            6,
            "QA Verification",
            StatusCategory::Review,
            BadgeVariant::Danger,
        ),
        (7, "Staging", StatusCategory::Review, BadgeVariant::Purple),
        (
            8,
            "Ready for Prod",
            StatusCategory::Done,
            BadgeVariant::Teal,
        ),
        (9, "Deployed", StatusCategory::Done, BadgeVariant::Success),
        (
            10,
            "Archived",
            StatusCategory::Archived,
            BadgeVariant::Default,
        ),
    ];
    let statuses: Vec<WorkflowStatusDto> = raw
        .iter()
        .map(|(id, name, cat, badge)| {
            create_sample_status(*id, name, *cat, *badge, None, *id == 1).with_task_count(1)
        })
        .collect();

    let tasks: Vec<Task> = raw
        .iter()
        .map(|(id, name, _, _)| {
            create_sample_task(
                *id * 10,
                &format!("Task for {name}"),
                TaskStatus::Accepted,
                Some(*id),
                vec![],
            )
        })
        .collect();

    BoardSnapshot {
        project_id: 3,
        project_name: "Custom 10-Status Pipeline".to_string(),
        enabled_views: ProjectViewKind::all().to_vec(),
        default_view: ProjectViewKind::KanbanClassic,
        caller_permissions: 63,
        statuses,
        tasks,
        dependencies: Vec::new(),
    }
}

/// Fixture 4: Soft WIP limit exceeded
fn fixture_wip_limit_exceeded() -> BoardSnapshot {
    let status = create_sample_status(
        20,
        "WIP Column",
        StatusCategory::InProgress,
        BadgeVariant::Warning,
        Some(2),
        false,
    )
    .with_task_count(4); // 4 tasks > limit 2 => wip_exceeded = true

    let tasks = vec![
        create_sample_task(201, "Task A", TaskStatus::InProgress, Some(20), vec![]),
        create_sample_task(202, "Task B", TaskStatus::InProgress, Some(20), vec![]),
        create_sample_task(203, "Task C", TaskStatus::InProgress, Some(20), vec![]),
        create_sample_task(204, "Task D", TaskStatus::InProgress, Some(20), vec![]),
    ];

    BoardSnapshot {
        project_id: 4,
        project_name: "WIP Limit Exceeded Project".to_string(),
        enabled_views: ProjectViewKind::all().to_vec(),
        default_view: ProjectViewKind::KanbanClassic,
        caller_permissions: 63,
        statuses: vec![status],
        tasks,
        dependencies: Vec::new(),
    }
}

/// Fixture 5: Scale test with 250 tasks
fn fixture_scale_250_tasks() -> BoardSnapshot {
    let statuses = vec![
        create_sample_status(
            1,
            "Backlog",
            StatusCategory::Todo,
            BadgeVariant::Default,
            None,
            true,
        ),
        create_sample_status(
            2,
            "Active",
            StatusCategory::InProgress,
            BadgeVariant::Info,
            None,
            false,
        ),
        create_sample_status(
            3,
            "Delivered",
            StatusCategory::Done,
            BadgeVariant::Success,
            None,
            false,
        ),
    ];

    let alice = User {
        id: 10,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };
    let bob = User {
        id: 20,
        name: "Bob".to_string(),
        email: "bob@example.com".to_string(),
    };

    let mut tasks = Vec::with_capacity(250);
    for i in 1..=250 {
        let ws_id = match i % 3 {
            1 => 1,
            2 => 2,
            _ => 3,
        };
        let assignees = match i % 3 {
            1 => vec![alice.clone()],
            2 => vec![bob.clone()],
            _ => vec![],
        };
        tasks.push(create_sample_task(
            i as i64,
            &format!("Scale Task #{i}"),
            TaskStatus::InProgress,
            Some(ws_id),
            assignees,
        ));
    }

    BoardSnapshot {
        project_id: 5,
        project_name: "250 Tasks Scale Project".to_string(),
        enabled_views: ProjectViewKind::all().to_vec(),
        default_view: ProjectViewKind::KanbanClassic,
        caller_permissions: 63,
        statuses,
        tasks,
        dependencies: Vec::new(),
    }
}

/// Fixture 6: Unicode, emojis, and markdown characters in task titles
fn fixture_unicode_markdown() -> BoardSnapshot {
    let statuses = vec![
        create_sample_status(
            1,
            "Incoming 📥",
            StatusCategory::Todo,
            BadgeVariant::Info,
            None,
            true,
        ),
        create_sample_status(
            2,
            "Done ✨",
            StatusCategory::Done,
            BadgeVariant::Success,
            None,
            false,
        ),
    ];

    let tasks = vec![
        create_sample_task(
            501,
            "🎯 Target: Release v1.0.0-rc.1",
            TaskStatus::Suggested,
            Some(1),
            vec![],
        ),
        create_sample_task(
            502,
            "**Crucial**: SQL & XSS escape test <script>",
            TaskStatus::Suggested,
            Some(1),
            vec![],
        ),
        create_sample_task(
            503,
            "日本語タスク: データベース接続確認",
            TaskStatus::Accepted,
            Some(1),
            vec![],
        ),
        create_sample_task(
            504,
            "Norwegian: Smørrebrød & Blåbærsyltetøy ÆØÅ",
            TaskStatus::Finished,
            Some(2),
            vec![],
        ),
    ];

    BoardSnapshot {
        project_id: 6,
        project_name: "Unicode & Markdown Project".to_string(),
        enabled_views: ProjectViewKind::all().to_vec(),
        default_view: ProjectViewKind::KanbanClassic,
        caller_permissions: 63,
        statuses,
        tasks,
        dependencies: Vec::new(),
    }
}

// ============================================================================
// TC2 Acceptance Criteria Tests
// ============================================================================

#[test]
fn test_tc2_all_views_render_empty_project() {
    let board = fixture_empty_project();
    for &view in ProjectViewKind::all() {
        let html = render_view_vdom(view, board.clone());
        assert!(!html.is_empty(), "View {:?} rendered empty HTML", view);
        // Assert no crashes and status columns or empty notice present
        match view {
            ProjectViewKind::KanbanClassic => {
                assert!(html.contains("To Do"));
                assert!(html.contains("In Progress"));
                assert!(html.contains("Done"));
                assert!(html.contains("No To Do tasks"));
            }
            ProjectViewKind::List => {
                assert!(html.contains("No tasks found in this project."));
            }
            ProjectViewKind::KanbanSwimlanes => {
                assert!(html.contains("Unassigned"));
            }
            ProjectViewKind::Gantt => {
                assert!(html.contains("Total Tasks"));
                assert!(html.contains("No tasks scheduled for timeline view."));
            }
        }
    }
}

#[test]
fn test_tc2_all_views_render_single_status() {
    let board = fixture_single_status();
    for &view in ProjectViewKind::all() {
        let html = render_view_vdom(view, board.clone());
        assert!(html.contains("101"), "Task 101 missing in {:?}", view);
        assert!(html.contains("102"), "Task 102 missing in {:?}", view);
        assert!(html.contains("103"), "Task 103 missing in {:?}", view);
    }
}

#[test]
fn test_tc2_all_views_render_ten_custom_statuses() {
    let board = fixture_ten_custom_statuses();
    for &view in ProjectViewKind::all() {
        let html = render_view_vdom(view, board.clone());
        assert!(html.contains("Triage"), "Triage missing in {:?}", view);
        assert!(html.contains("Farming"), "Farming missing in {:?}", view);
        assert!(html.contains("Deployed"), "Deployed missing in {:?}", view);
    }
}

#[test]
fn test_tc2_wip_warning_styles_emitted_when_exceeded() {
    let board_exceeded = fixture_wip_limit_exceeded();

    // In KanbanClassicView, wip-warning must appear when wip_exceeded is true
    let html_exceeded = render_view_vdom(ProjectViewKind::KanbanClassic, board_exceeded);
    assert!(
        html_exceeded.contains("wip-warning"),
        "wip-warning class must appear when wip_exceeded is true"
    );
    assert!(
        html_exceeded.contains("⚠️ WIP Exceeded"),
        "WIP Exceeded badge must appear when wip_exceeded is true"
    );

    // In a normal fixture without WIP exceeded, wip-warning must NOT appear
    let board_normal = fixture_empty_project();
    let html_normal = render_view_vdom(ProjectViewKind::KanbanClassic, board_normal);
    assert!(
        !html_normal.contains("wip-warning"),
        "wip-warning must NOT appear when wip_exceeded is false"
    );
}

#[test]
fn test_tc2_scale_250_tasks_rendered_once_in_kanban_and_list() {
    let board = fixture_scale_250_tasks();

    // 1. KanbanClassicView: Every task ID 1..=250 rendered exactly once
    let kanban_html = render_view_vdom(ProjectViewKind::KanbanClassic, board.clone());
    for id in 1..=250 {
        let expected_tag = format!("data-task-id=\"{id}\"");
        let matches: Vec<_> = kanban_html.match_indices(&expected_tag).collect();
        assert_eq!(
            matches.len(),
            1,
            "Task #{id} must be rendered exactly once in KanbanClassicView (found {})",
            matches.len()
        );
    }

    // 2. ListView: Every task ID 1..=250 rendered exactly once
    let list_html = render_view_vdom(ProjectViewKind::List, board.clone());
    for id in 1..=250 {
        let expected_tag = format!("data-task-id=\"{id}\"");
        let matches: Vec<_> = list_html.match_indices(&expected_tag).collect();
        assert_eq!(
            matches.len(),
            1,
            "Task #{id} must be rendered exactly once in ListView (found {})",
            matches.len()
        );
    }

    // 3. KanbanSwimlanesView: Every task ID 1..=250 rendered
    let swimlanes_html = render_view_vdom(ProjectViewKind::KanbanSwimlanes, board.clone());
    for id in 1..=250 {
        let expected_tag = format!("data-task-id=\"{id}\"");
        assert!(
            swimlanes_html.contains(&expected_tag),
            "Task #{id} missing in KanbanSwimlanesView"
        );
    }
}

#[test]
fn test_tc2_unicode_and_markdown_titles_properly_escaped() {
    let board = fixture_unicode_markdown();
    for &view in ProjectViewKind::all() {
        let html = render_view_vdom(view, board.clone());

        // Emoji & non-ASCII preserved
        assert!(
            html.contains("🎯 Target: Release v1.0.0-rc.1"),
            "Target release title missing in {:?}",
            view
        );
        assert!(
            html.contains("日本語タスク: データベース接続確認"),
            "Japanese title missing in {:?}",
            view
        );
        assert!(
            html.contains("Smørrebrød") && html.contains("Blåbærsyltetøy") && html.contains("ÆØÅ"),
            "Norwegian characters missing in {:?}: {}",
            view,
            html
        );

        // Raw unescaped <script> tag must NEVER appear in SSR output
        assert!(
            !html.contains("<script>"),
            "XSS vulnerability: unescaped <script> tag in {:?}",
            view
        );
    }
}

#[test]
fn test_tc2_only_allowed_badge_variant_classes_emitted() {
    let board = fixture_ten_custom_statuses();
    let html = render_view_vdom(ProjectViewKind::KanbanClassic, board);

    let allowed = [
        "badge-variant-default",
        "badge-variant-info",
        "badge-variant-success",
        "badge-variant-warning",
        "badge-variant-danger",
        "badge-variant-purple",
        "badge-variant-teal",
    ];

    for variant in allowed {
        assert!(
            html.contains(variant),
            "Expected badge variant class '{variant}' to be emitted"
        );
    }
}
