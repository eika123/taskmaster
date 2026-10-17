use crate::backend::app_endpoints::logout;
use crate::types::User;
use crate::Route;
use dioxus::prelude::*;

#[derive(Clone, Copy)]
pub struct AuthState {
    pub user: Signal<Option<User>>,
    pub token: Signal<Option<String>>,
    #[allow(dead_code)]
    pub is_loading: Signal<bool>,
}

impl AuthState {
    pub fn login_user(&mut self, user: User, token: String) {
        self.user.set(Some(user));
        self.token.set(Some(token.clone()));
        document::eval(&format!(
            r#"localStorage.setItem("flowtask_session_token", "{}");"#,
            token
        ));
    }

    pub fn logout_user(&mut self) {
        let current_token = (self.token)();
        self.user.set(None);
        self.token.set(None);
        document::eval(r#"localStorage.removeItem("flowtask_session_token");"#);

        // request to delete token server side.
        if let Some(token) = current_token {
            spawn(async move {
                let _ = logout(Some(token)).await;
            });
        }
    }
}

/// Layout component, helps redirect unauthenticated users back
/// to login from routes requiring authentication.
/// Also prevents displaying "stale" components that can't fetch
/// data from backend when not authenticated
///
/// Does NOT protect against anything or provide any sort of security
/// against the backend. Authentication
/// checks against backend are placed in server-side router middleware.
#[component]
pub fn AuthLayout() -> Element {
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
