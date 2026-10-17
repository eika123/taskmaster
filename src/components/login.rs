use crate::auth::AuthState;
use crate::backend::app_endpoints::login;
use crate::Route;
use dioxus::prelude::*;

/// Login page component
#[component]
pub fn LoginPage() -> Element {
    let mut auth = use_context::<AuthState>();
    let mut email = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut error_msg = use_signal(|| Option::<String>::None);
    let mut is_submitting = use_signal(|| false);

    let navigator = use_navigator();

    let on_submit = move |evt: FormEvent| {
        evt.prevent_default();
        if is_submitting() {
            return;
        }

        let email_val = email().trim().to_string();
        let pass_val = password();

        if email_val.is_empty() || pass_val.is_empty() {
            error_msg.set(Some("Please fill in all fields.".to_string()));
            return;
        }

        is_submitting.set(true);
        error_msg.set(None);

        let nav = navigator.clone();
        spawn(async move {
            match login(email_val, pass_val).await {
                Ok(resp) => {
                    auth.login_user(resp.user, resp.token);
                    is_submitting.set(false);
                    nav.push(Route::TasksDashboardPage {});
                }
                Err(err) => {
                    is_submitting.set(false);
                    error_msg.set(Some(err.to_string()));
                }
            }
        });
    };

    rsx! {
        div { class: "auth-page-container",
            div { class: "auth-card",
                div { class: "auth-header",
                    span { class: "auth-icon", "🔐" }
                    h1 { class: "auth-title", "Log in to FlowTask" }
                    p { class: "auth-subtitle", "Welcome back! Please enter your details." }
                }

                if let Some(err) = error_msg() {
                    div { class: "auth-error-banner",
                        span { class: "auth-error-icon", "⚠️" }
                        span { "{err}" }
                    }
                }

                form {
                    class: "auth-form",
                    action: "javascript:void(0);",
                    onsubmit: on_submit,
                    div { class: "form-group",
                        label { r#for: "login-email", "Email address" }
                        input {
                            id: "login-email",
                            r#type: "email",
                            placeholder: "you@example.com",
                            required: true,
                            value: "{email}",
                            oninput: move |e| email.set(e.value()),
                            disabled: is_submitting(),
                        }
                    }

                    div { class: "form-group",
                        label { r#for: "login-password", "Password" }
                        input {
                            id: "login-password",
                            r#type: "password",
                            placeholder: "••••••••",
                            required: true,
                            value: "{password}",
                            oninput: move |e| password.set(e.value()),
                            disabled: is_submitting(),
                        }
                    }

                    button {
                        r#type: "submit",
                        class: "btn-auth-submit",
                        disabled: is_submitting(),
                        if is_submitting() {
                            "Signing in..."
                        } else {
                            "Sign In"
                        }
                    }
                }

                div { class: "auth-footer",
                    p {
                        "Don't have an account? "
                        Link { to: Route::RegisterPage {}, class: "auth-link", "Create one here" }
                    }
                }
            }
        }
    }
}
