use crate::auth::AuthState;
use crate::backend::app_endpoints::login;
use crate::types::is_valid_email;
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
    let mut email_touched = use_signal(|| false);

    let navigator = use_navigator();

    let email_is_invalid = use_memo(move || {
        if !email_touched() {
            return false;
        }
        let val = email().trim().to_string();
        !val.is_empty() && !is_valid_email(&val)
    });

    let on_submit = move |evt: FormEvent| {
        evt.prevent_default();
        if is_submitting() {
            return;
        }

        email_touched.set(true);

        let email_val = email().trim().to_string();
        let pass_val = password();

        if email_val.is_empty() || pass_val.is_empty() {
            error_msg.set(Some("Please fill in all fields.".to_string()));
            return;
        }

        if !is_valid_email(&email_val) {
            error_msg.set(Some("Please enter a valid email address.".to_string()));
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
                LoginHeader {}

                if let Some(err) = error_msg() {
                    div { class: "auth-error-banner",
                        span { class: "auth-error-icon", "⚠️" }
                        span { "{err}" }
                    }
                }

                LoginForm {
                    email: email(),
                    password: password(),
                    is_submitting: is_submitting(),
                    email_is_invalid: email_is_invalid(),
                    on_email_input: move |v| email.set(v),
                    on_email_blur: move |_| email_touched.set(true),
                    on_password_input: move |v| password.set(v),
                    on_submit,
                }

                LoginFooter {}
            }
        }
    }
}

/// Header for the login card with icon and welcome text.
#[component]
fn LoginHeader() -> Element {
    rsx! {
        div { class: "auth-header",
            span { class: "auth-icon", "🔐" }
            h1 { class: "auth-title", "Log in to FlowTask" }
            p { class: "auth-subtitle", "Welcome back! Please enter your details." }
        }
    }
}

/// Form inputs and submit button for user login.
#[component]
fn LoginForm(
    email: String,
    password: String,
    is_submitting: bool,
    email_is_invalid: bool,
    on_email_input: EventHandler<String>,
    on_email_blur: EventHandler<()>,
    on_password_input: EventHandler<String>,
    on_submit: EventHandler<FormEvent>,
) -> Element {
    rsx! {
        form {
            class: "auth-form",
            action: "javascript:void(0);",
            onsubmit: move |evt| on_submit.call(evt),
            div { class: "form-group",
                label { r#for: "login-email", "Email address" }
                input {
                    id: "login-email",
                    r#type: "email",
                    class: if email_is_invalid { "input-invalid" },
                    placeholder: "you@example.com",
                    required: true,
                    value: "{email}",
                    oninput: move |e| on_email_input.call(e.value()),
                    onblur: move |_| on_email_blur.call(()),
                    disabled: is_submitting,
                }
                span {
                    class: "field-hint-error",
                    style: if !email_is_invalid { "visibility: hidden;" },
                    "Please enter a valid email address."
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
                    oninput: move |e| on_password_input.call(e.value()),
                    disabled: is_submitting,
                }
            }

            button {
                r#type: "submit",
                class: "btn-auth-submit",
                disabled: is_submitting,
                if is_submitting { "Signing in..." } else { "Sign In" }
            }
        }
    }
}

/// Footer with registration link for new users.
#[component]
fn LoginFooter() -> Element {
    rsx! {
        div { class: "auth-footer",
            p {
                "Don't have an account? "
                Link { to: Route::RegisterPage {}, class: "auth-link", "Create one here" }
            }
        }
    }
}
