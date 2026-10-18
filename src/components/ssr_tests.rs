use dioxus::prelude::*;
use dioxus::history::{History, MemoryHistory};
use dioxus::router::components::HistoryProvider;
use std::rc::Rc;
use crate::Route;
use crate::components::*;

#[component]
fn TestSsrApp(initial_path: String, user: Option<User>) -> Element {
    let user_sig = use_signal(|| user);
    let token_sig = use_signal(|| None::<String>);
    let is_loading_sig = use_signal(|| false);

    use_context_provider(|| AuthState {
        user: user_sig,
        token: token_sig,
        is_loading: is_loading_sig,
    });

    rsx! {
        HistoryProvider {
            history: move |_| {
                Rc::new(MemoryHistory::with_initial_path(initial_path.clone())) as Rc<dyn History>
            },
            Router::<Route> {}
        }
    }
}

fn render_route_ssr(path: &str, user: Option<User>) -> String {
    let mut vdom = VirtualDom::new_with_props(
        TestSsrApp,
        TestSsrAppProps {
            initial_path: path.to_string(),
            user,
        },
    );
    vdom.rebuild_in_place();
    dioxus::ssr::render(&vdom)
}

#[derive(Clone, Routable, PartialEq)]
enum TestProjectsRoute {
    #[route("/")]
    TestProjectPage {},
}

#[component]
fn TestProjectPage() -> Element {
    let projects = use_context::<Signal<Vec<Project>>>();
    rsx! {
        DashboardProjects {
            projects: projects(),
            on_invite: |_| {},
        }
    }
}

#[component]
fn TestProjectsWrapper(projects: Vec<Project>) -> Element {
    let projects_sig = use_signal(|| projects);
    use_context_provider(|| projects_sig);

    rsx! {
        Router::<TestProjectsRoute> {}
    }
}

fn render_projects_ssr(projects: Vec<Project>) -> String {
    let mut vdom = VirtualDom::new_with_props(
        TestProjectsWrapper,
        TestProjectsWrapperProps { projects },
    );
    vdom.rebuild_in_place();
    dioxus::ssr::render(&vdom)
}

#[test]
fn test_ssr_navbar_unauthenticated() {
    let html = render_route_ssr("/", None);

    // Brand and Home
    assert!(html.contains("FlowTask"));
    assert!(html.contains(r#"href="/" class="brand-link""#));
    assert!(html.contains(r#"href="/" class="nav-link""#));
    assert!(html.contains(">Home<"));

    // Unauthenticated buttons
    assert!(html.contains(r#"href="/login" class="btn-navbar-login""#));
    assert!(html.contains(">Log In<"));
    assert!(html.contains(r#"href="/register" class="btn-navbar-register""#));
    assert!(html.contains(">Sign Up<"));

    // Authenticated items must NOT be present
    assert!(!html.contains(r#"href="/dashboard""#));
    assert!(!html.contains("btn-navbar-logout"));
}

#[test]
fn test_ssr_navbar_authenticated() {
    let user = User {
        id: 1,
        name: "Alice Developer".into(),
        email: "alice@example.com".into(),
    };
    let html = render_route_ssr("/", Some(user));

    // Home link
    assert!(html.contains(r#"href="/" class="nav-link""#));
    assert!(html.contains(">Home<"));

    // Authenticated nav links
    assert!(html.contains(r#"href="/dashboard" class="nav-link""#));
    assert!(html.contains(">Dashboard<"));
    assert!(html.contains(r#"href="/tasks" class="nav-link""#));
    assert!(html.contains(">My Tasks<"));

    // User profile badge and logout button
    assert!(html.contains("Alice Developer"));
    assert!(html.contains(r#"class="user-avatar-circle">A<"#));
    assert!(html.contains("btn-navbar-logout"));
    assert!(html.contains(">Log Out<"));

    // Unauthenticated buttons must NOT be present
    assert!(!html.contains("btn-navbar-login"));
    assert!(!html.contains("btn-navbar-register"));
}

#[test]
fn test_ssr_login_page() {
    let html = render_route_ssr("/login", None);

    // Form inputs
    assert!(html.contains(r#"id="login-email""#));
    assert!(html.contains(r#"id="login-password""#));

    // Sign in submit button
    assert!(html.contains(r#"type="submit""#));
    assert!(html.contains("btn-auth-submit"));
    assert!(html.contains(">Sign In<"));

    // Registration link
    assert!(html.contains(r#"href="/register" class="auth-link""#));
    assert!(html.contains(">Create one here<"));
}

#[test]
fn test_ssr_register_page() {
    let html = render_route_ssr("/register", None);

    // Form inputs
    assert!(html.contains(r#"id="register-name""#));
    assert!(html.contains(r#"id="register-email""#));
    assert!(html.contains(r#"id="register-password""#));
    assert!(html.contains(r#"id="register-confirm-password""#));

    // Create Account submit button
    assert!(html.contains(r#"type="submit""#));
    assert!(html.contains("btn-auth-submit"));
    assert!(html.contains(">Create Account<"));

    // Login link
    assert!(html.contains(r#"href="/login" class="auth-link""#));
    assert!(html.contains(">Log in here<"));
}

#[test]
fn test_ssr_landing_page_buttons() {
    let html = render_route_ssr("/", None);

    // Hero headline
    assert!(html.contains("Organize, Collaborate, and"));
    assert!(html.contains("Ship Faster."));

    // Action buttons / links
    assert!(html.contains(r#"href="/tasks" class="btn-hero-primary""#));
    assert!(html.contains(">Open Task Board →<"));
    assert!(html.contains(r#"href="/task/new" class="btn-hero-secondary""#));
    assert!(html.contains(">+ Create Task<"));
    assert!(html.contains(r#"href="/tasks" class="btn-cta""#));
    assert!(html.contains(">Launch Dashboard<"));
}

#[test]
fn test_ssr_my_projects_with_projects() {
    let projects = vec![
        Project {
            id: 42,
            name: "Acme Engineering".into(),
            description: "Core services and APIs".into(),
            owner_org_id: Some(10),
            owner_org_name: Some("Acme Corp".into()),
            leader_id: 1,
            leader_name: "Alice Developer".into(),
            created_at: 0,
        },
        Project {
            id: 43,
            name: "Mobile App".into(),
            description: "iOS and Android client".into(),
            owner_org_id: None,
            owner_org_name: None,
            leader_id: 2,
            leader_name: "Bob".into(),
            created_at: 0,
        },
    ];

    let html = render_projects_ssr(projects);

    // Header count
    assert!(html.contains("My Projects (2)"));

    // Project cards
    assert!(html.contains("Acme Engineering"));
    assert!(html.contains(r#"href="/projects/tasks/42""#));
    assert!(html.contains("Acme Corp"));

    assert!(html.contains("Mobile App"));
    assert!(html.contains(r#"href="/projects/tasks/43""#));
    assert!(html.contains("Standalone"));

    // Action buttons and links
    assert!(html.contains("+ Invite"));
    assert!(html.contains("btn-action-sm"));
    assert!(html.contains("View Board →"));
}

#[test]
fn test_ssr_my_projects_empty() {
    let html = render_projects_ssr(vec![]);

    // Header count 0
    assert!(html.contains("My Projects (0)"));

    // Empty state message
    assert!(html.contains("You are not a member of any projects yet. Create one above!"));
    assert!(!html.contains("+ Invite"));
}
