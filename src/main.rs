use dioxus::prelude::*;
pub mod types;
pub mod backend;
pub mod components;
use crate::components::*;
pub use crate::types::*;


pub use crate::backend::app_endpoints::*;

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

        #[layout(AuthGuard)]
            #[route("/tasks")]
            TasksDashboardPage {},
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

/// Layout component protecting authenticated routes.
#[component]
fn AuthGuard() -> Element {
    let auth = use_context::<AuthState>();
    let user = (auth.user)();
    let is_loading = (auth.is_loading)();

    if is_loading {
        return rsx! {
            div { class: "auth-guard-loading",
                p { "Verifying authentication..." }
            }
        };
    }

    if user.is_some() {
        rsx! {
            Outlet::<Route> {}
        }
    } else {
        rsx! {
            div { class: "unauthorized-container",
                div { class: "unauthorized-card",
                    span { class: "unauthorized-icon", "🔒" }
                    h2 { class: "unauthorized-title", "Authentication Required" }
                    p { class: "unauthorized-desc",
                        "You must be logged in to view and manage tasks on the task board."
                    }
                    div { class: "unauthorized-actions",
                        Link {
                            to: Route::LoginPage {},
                            class: "btn-unauthorized-login",
                            "Log In"
                        }
                        Link {
                            to: Route::LandingPage {},
                            class: "btn-unauthorized-home",
                            "Back to Home"
                        }
                    }
                }
            }
        }
    }
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

// /// Task board dashboard page
#[component]
fn TasksDashboardPage() -> Element {
    rsx! {
        TaskView {}
    }
}

/// Shared navigation header.
#[component]
fn Navbar() -> Element {
    let mut auth = use_context::<AuthState>();
    let current_user = (auth.user)();

    rsx! {
        header { class: "app-header",
            div { class: "app-navbar",
                div { class: "navbar-brand",
                    Link { to: Route::LandingPage {}, class: "brand-link",
                        span { class: "brand-logo", "⚡" }
                        span { class: "brand-name", "FlowTask" }
                    }
                }
                nav { class: "navbar-links",
                    Link { to: Route::LandingPage {}, class: "nav-link", "Home" }
                    Link { to: Route::TasksDashboardPage {}, class: "nav-link", "Task Board" }
                }

                div { class: "navbar-actions",
                    if let Some(user) = current_user {
                        div { class: "user-profile-badge",
                            span { class: "user-avatar-circle",
                                "{user.name.chars().next().unwrap_or('U')}"
                            }
                            span { class: "user-name-text", "{user.name}" }
                            button {
                                r#type: "button",
                                class: "btn-navbar-logout",
                                onclick: move |_| auth.logout_user(),
                                "Log Out"
                            }
                        }
                    } else {
                        div { class: "auth-nav-buttons",
                            Link {
                                to: Route::LoginPage {},
                                class: "btn-navbar-login",
                                "Log In"
                            }
                            Link {
                                to: Route::RegisterPage {},
                                class: "btn-navbar-register",
                                "Sign Up"
                            }
                        }
                    }
                }
            }
        }

        main { class: "app-main-content", Outlet::<Route> {} }
    }
}