use dioxus::prelude::*;
use crate::Route;

#[component]
pub fn LandingPage() -> Element {
    rsx! {
        div { class: "landing-container",
            // Hero Section
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

            // Feature Highlights Grid
            section { class: "landing-features",
                div { class: "features-header",
                    h2 { class: "features-title", "Built for Efficient Engineering Workflows" }
                    p { class: "features-desc",
                        "Everything you need to keep projects on track from suggestion to completion."
                    }
                }

                div { class: "features-grid",
                    div { class: "feature-card",
                        div { class: "feature-icon", "📋" }
                        h3 { class: "feature-card-title", "Kanban & Flexible Views" }
                        p { class: "feature-card-text",
                            "Toggle between Suggested, Accepted, In Progress, and Finished columns with interactive column filters."
                        }
                    }

                    div { class: "feature-card",
                        div { class: "feature-icon", "👥" }
                        h3 { class: "feature-card-title", "Team Assignment & Search" }
                        p { class: "feature-card-text",
                            "Quickly find and assign teammates with auto-complete search. Built-in SQLite constraints guarantee assignment integrity."
                        }
                    }

                    div { class: "feature-card",
                        div { class: "feature-icon", "🔍" }
                        h3 { class: "feature-card-title", "Deep Task Specifications" }
                        p { class: "feature-card-text",
                            "Click into dedicated task pages to write descriptions, view assignees, and manage workflow status."
                        }
                    }

                    div { class: "feature-card",
                        div { class: "feature-icon", "⚡" }
                        h3 { class: "feature-card-title", "Fullstack Rust & SQLite" }
                        p { class: "feature-card-text",
                            "Powered by Dioxus 0.7 server functions, reactive signals, and transactional SQLite database persistence."
                        }
                    }
                }
            }

            // Call to Action Banner
            section { class: "landing-cta-banner",
                div { class: "cta-content",
                    h2 { "Ready to get things done?" }
                    p { "Jump straight into your task board or draft a new task suggestion." }
                }
                Link { to: Route::TasksDashboardPage {}, class: "btn-cta", "Launch Dashboard" }
            }
        }
    }
}

