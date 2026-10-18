use crate::Route;
use dioxus::prelude::*;

#[component]
pub fn LandingPage() -> Element {
    rsx! {
        div { class: "landing-container",
            LandingHero {}
            LandingFeatures {}
            LandingCtaBanner {}
        }
    }
}

/// Hero banner highlighting product purpose and primary call-to-actions.
#[component]
fn LandingHero() -> Element {
    rsx! {
        section { class: "landing-hero",
            div { class: "hero-badge", "Modern Project & Task Management" }
            h1 { class: "hero-title",
                "Organize, Collaborate, and "
                span { class: "gradient-text", "Ship Faster." }
            }
            p { class: "hero-subtitle",
                "A fast, responsive fullstack task board built with Rust and Dioxus. Track work across flexible status columns, search and assign team members, and manage rich task specifications."
            }
            div { class: "hero-actions",
                Link {
                    to: Route::TasksDashboardPage {},
                    class: "btn-hero-primary",
                    "Open Task Board →"
                }
                Link {
                    to: Route::NewTaskPage {},
                    class: "btn-hero-secondary",
                    "+ Create Task"
                }
            }
        }
    }
}

/// Grid highlighting key features of the application.
#[component]
fn LandingFeatures() -> Element {
    rsx! {
        section { class: "landing-features",
            div { class: "features-header",
                h2 { class: "features-title", "Built for Efficient Engineering Workflows" }
                p { class: "features-desc",
                    "Everything you need to keep projects on track from suggestion to completion."
                }
            }

            div { class: "features-grid",
                FeatureCard {
                    icon: "📋",
                    title: "Kanban & Flexible Views",
                    desc: "Toggle between Suggested, Accepted, In Progress, and Finished columns with interactive column filters.",
                }
                FeatureCard {
                    icon: "👥",
                    title: "Team Assignment & Search",
                    desc: "Quickly find and assign teammates with auto-complete search. Built-in SQLite constraints guarantee assignment integrity.",
                }
                FeatureCard {
                    icon: "🔍",
                    title: "Deep Task Specifications",
                    desc: "Click into dedicated task pages to write descriptions, view assignees, and manage workflow status.",
                }
                FeatureCard {
                    icon: "⚡",
                    title: "Fullstack Rust & SQLite",
                    desc: "Powered by Dioxus 0.7 server functions, reactive signals, and transactional SQLite database persistence.",
                }
            }
        }
    }
}

/// Individual feature highlight card.
#[component]
fn FeatureCard(icon: &'static str, title: &'static str, desc: &'static str) -> Element {
    rsx! {
        div { class: "feature-card",
            div { class: "feature-icon", "{icon}" }
            h3 { class: "feature-card-title", "{title}" }
            p { class: "feature-card-text", "{desc}" }
        }
    }
}

/// Bottom call to action banner inviting the user to jump into the dashboard.
#[component]
fn LandingCtaBanner() -> Element {
    rsx! {
        section { class: "landing-cta-banner",
            div { class: "cta-content",
                h2 { "Ready to get things done?" }
                p { "Jump straight into your task board or draft a new task suggestion." }
            }
            Link { to: Route::TasksDashboardPage {}, class: "btn-cta", "Launch Dashboard" }
        }
    }
}
