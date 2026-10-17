use dioxus::prelude::*;
use crate::components::*;
pub use crate::types::*;
pub use crate::Route;


pub use crate::backend::app_endpoints::*;

/// Shared navigation header.
#[component]
pub fn Navbar() -> Element {
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
                    if current_user.is_some() {
                        Link { to: Route::DashboardPage {}, class: "nav-link", "Dashboard" }
                        Link {
                            to: Route::TasksDashboardPage {},
                            class: "nav-link",
                            "My Tasks"
                        }
                    }
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