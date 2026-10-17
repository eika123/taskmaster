use dioxus::prelude::*;

pub mod types;
pub mod backend;
pub mod components;
use crate::components::*;
pub use crate::types::*;


pub use crate::backend::app_endpoints::*;

use crate::components::navbar::Navbar;

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[layout(Navbar)]
        #[route("/")]
        LandingPage {},
        #[route("/login")]
        LoginPage {},
        #[route("/register")]
        RegisterPage {},

        #[layout(AuthLayout)]
            #[route("/dashboard")]
            DashboardPage {},
            #[route("/organizations/:organization_id")]
            OrganizationDashboardPage { organization_id: i64 },
            #[route("/tasks")]
            TasksDashboardPage {},
            #[route("/projects/tasks/:project_id")]
            ProjectTasksPage { project_id: i64 },
            #[route("/task/new")]
            NewTaskPage {},
            #[route("/task/:id")]
            TaskPage { id: i64 },
}

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/main.css");
const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");

#[cfg(feature = "server")]
fn main() {
    dioxus::serve(|| async move {
        let pool = crate::types::get_db().await.clone();
        let session_store = crate::types::create_session_store(pool.clone()).await?;
        let auth_layer = crate::types::create_auth_layer(pool);
        let session_layer = axum_session::SessionLayer::new(session_store);

        let app_router = dioxus::server::router(App)
            .layer(axum::middleware::from_fn(crate::types::auth_guard_middleware))
            .layer(auth_layer)
            .layer(session_layer)
            .layer(axum::middleware::from_fn(crate::types::auth_header_to_cookie_middleware));

        Ok(app_router)
    });
}

#[cfg(not(feature = "server"))]
fn main() {
    dioxus::launch(App);
}



#[component]
fn App() -> Element {
    let mut user = use_signal(|| None::<crate::types::User>);
    let mut token = use_signal(|| None::<String>);
    let mut is_loading = use_signal(|| true);

    let auth_state = AuthState {
        user,
        token,
        is_loading,
    };
    use_context_provider(|| auth_state);

    // Restore user session from localStorage
    use_effect(move || {
        spawn(async move {
            let mut eval = document::eval(r#"
                let t = localStorage.getItem("flowtask_session_token");
                dioxus.send(t);
            "#);
            if let Ok(Some(t_owned)) = eval.recv::<Option<String>>().await {
                if !t_owned.is_empty() {
                    token.set(Some(t_owned.clone()));
                    if let Ok(Some(u)) = crate::backend::app_endpoints::get_current_user(Some(t_owned)).await {
                        user.set(Some(u));
                    } else {
                        token.set(None);
                        let _ = document::eval(r#"localStorage.removeItem("flowtask_session_token");"#);
                    }
                }
            }
            is_loading.set(false);
        });
    });

    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        document::Link { rel: "stylesheet", href: TAILWIND_CSS }
        Router::<Route> {}
    }
}

// /// My Tasks dashboard page (only tasks assigned to or authored by current user)
#[component]
fn TasksDashboardPage() -> Element {
    rsx! {
        TaskView { project_id: None }
    }
}

// /// Project-scoped Kanban board page
#[component]
fn ProjectTasksPage(project_id: i64) -> Element {
    rsx! {
        TaskView { project_id: Some(project_id) }
    }
}

