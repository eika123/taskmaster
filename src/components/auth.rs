use dioxus::prelude::*;
use crate::backend::app_endpoints::{login, logout, register};
use crate::types::User;
use crate::Route;

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
        document::eval(&format!(r#"localStorage.setItem("flowtask_session_token", "{}");"#, token));
    }

    pub fn logout_user(&mut self) {
        let current_token = (self.token)();
        self.user.set(None);
        self.token.set(None);
        document::eval(r#"localStorage.removeItem("flowtask_session_token");"#);

        if let Some(token) = current_token {
            spawn(async move {
                let _ = logout(Some(token)).await;
            });
        }
    }
}

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

/// Register page component
#[component]
pub fn RegisterPage() -> Element {
    let mut auth = use_context::<AuthState>();
    let mut name = use_signal(String::new);
    let mut email = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut confirm_password = use_signal(String::new);
    let mut error_msg = use_signal(|| Option::<String>::None);
    let mut is_submitting = use_signal(|| false);

    let navigator = use_navigator();

    let on_submit = move |evt: FormEvent| {
        evt.prevent_default();
        if is_submitting() {
            return;
        }

        let name_val = name().trim().to_string();
        let email_val = email().trim().to_string();
        let pass_val = password();
        let confirm_val = confirm_password();

        if name_val.is_empty() || email_val.is_empty() || pass_val.is_empty() {
            error_msg.set(Some("Please fill in all fields.".to_string()));
            return;
        }

        if pass_val != confirm_val {
            error_msg.set(Some("Passwords do not match.".to_string()));
            return;
        }

        if pass_val.len() < 6 {
            error_msg.set(Some("Password must be at least 6 characters.".to_string()));
            return;
        }

        is_submitting.set(true);
        error_msg.set(None);

        let nav = navigator.clone();
        spawn(async move {
            match register(name_val, email_val, pass_val).await {
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
                    span { class: "auth-icon", "✨" }
                    h1 { class: "auth-title", "Create an Account" }
                    p { class: "auth-subtitle", "Join FlowTask to manage and track team tasks." }
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
                        label { r#for: "register-name", "Full Name" }
                        input {
                            id: "register-name",
                            r#type: "text",
                            placeholder: "Jane Doe",
                            required: true,
                            value: "{name}",
                            oninput: move |e| name.set(e.value()),
                            disabled: is_submitting(),
                        }
                    }

                    div { class: "form-group",
                        label { r#for: "register-email", "Email address" }
                        input {
                            id: "register-email",
                            r#type: "email",
                            placeholder: "jane@example.com",
                            required: true,
                            value: "{email}",
                            oninput: move |e| email.set(e.value()),
                            disabled: is_submitting(),
                        }
                    }

                    div { class: "form-group",
                        label { r#for: "register-password", "Password" }
                        input {
                            id: "register-password",
                            r#type: "password",
                            placeholder: "•••••••• (min 6 chars)",
                            required: true,
                            value: "{password}",
                            oninput: move |e| password.set(e.value()),
                            disabled: is_submitting(),
                        }
                    }

                    div { class: "form-group",
                        label { r#for: "register-confirm-password", "Confirm Password" }
                        input {
                            id: "register-confirm-password",
                            r#type: "password",
                            placeholder: "••••••••",
                            required: true,
                            value: "{confirm_password}",
                            oninput: move |e| confirm_password.set(e.value()),
                            disabled: is_submitting(),
                        }
                    }

                    button {
                        r#type: "submit",
                        class: "btn-auth-submit",
                        disabled: is_submitting(),
                        if is_submitting() {
                            "Creating account..."
                        } else {
                            "Create Account"
                        }
                    }
                }

                div { class: "auth-footer",
                    p {
                        "Already have an account? "
                        Link { to: Route::LoginPage {}, class: "auth-link", "Log in here" }
                    }
                }
            }
        }
    }
}
