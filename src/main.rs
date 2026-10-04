use dioxus::prelude::*;
mod backend;
mod components;
use crate::components::*;

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[layout(Navbar)]
    #[route("/")]
    LandingPage {},
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

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
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
            
            // div { class: "navbar-actions",
            //     Link { to: Route::NewTaskPage {}, class: "btn-navbar-new", "+ New Task" }
            // }
            }
        }

        main { class: "app-main-content", Outlet::<Route> {} }
    }
}