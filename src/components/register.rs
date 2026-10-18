use crate::auth::AuthState;
use crate::backend::app_endpoints::register;
use crate::types::is_valid_email;
use crate::Route;
use dioxus::prelude::*;

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
    let mut email_touched = use_signal(|| false);
    let mut confirm_touched = use_signal(|| false);

    let navigator = use_navigator();

    let email_is_invalid = use_memo(move || {
        if !email_touched() {
            return false;
        }
        let val = email().trim().to_string();
        !val.is_empty() && !is_valid_email(&val)
    });

    let passwords_mismatch = use_memo(move || {
        if !confirm_touched() {
            return false;
        }
        let p = password();
        let cp = confirm_password();
        !cp.is_empty() && p != cp
    });

    let on_submit = move |evt: FormEvent| {
        evt.prevent_default();
        if is_submitting() {
            return;
        }

        email_touched.set(true);
        confirm_touched.set(true);

        let name_val = name().trim().to_string();
        let email_val = email().trim().to_string();
        let pass_val = password();
        let confirm_val = confirm_password();

        if name_val.is_empty() || email_val.is_empty() || pass_val.is_empty() {
            error_msg.set(Some("Please fill in all fields.".to_string()));
            return;
        }

        if !is_valid_email(&email_val) {
            error_msg.set(Some("Please enter a valid email address.".to_string()));
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
                RegisterHeader {}

                if let Some(err) = error_msg() {
                    div { class: "auth-error-banner",
                        span { class: "auth-error-icon", "⚠️" }
                        span { "{err}" }
                    }
                }

                RegisterForm {
                    name: name(),
                    email: email(),
                    password: password(),
                    confirm_password: confirm_password(),
                    is_submitting: is_submitting(),
                    email_is_invalid: email_is_invalid(),
                    passwords_mismatch: passwords_mismatch(),
                    on_name_input: move |v| name.set(v),
                    on_email_input: move |v| email.set(v),
                    on_email_blur: move |_| email_touched.set(true),
                    on_password_input: move |v| password.set(v),
                    on_confirm_input: move |v| confirm_password.set(v),
                    on_confirm_blur: move |_| confirm_touched.set(true),
                    on_submit,
                }

                RegisterFooter {}
            }
        }
    }
}

/// Header for the registration card with icon and welcome text.
#[component]
fn RegisterHeader() -> Element {
    rsx! {
        div { class: "auth-header",
            span { class: "auth-icon", "✨" }
            h1 { class: "auth-title", "Create an Account" }
            p { class: "auth-subtitle", "Join FlowTask to manage and track team tasks." }
        }
    }
}

/// Form inputs and submit button for user registration.
#[component]
fn RegisterForm(
    name: String,
    email: String,
    password: String,
    confirm_password: String,
    is_submitting: bool,
    email_is_invalid: bool,
    passwords_mismatch: bool,
    on_name_input: EventHandler<String>,
    on_email_input: EventHandler<String>,
    on_email_blur: EventHandler<()>,
    on_password_input: EventHandler<String>,
    on_confirm_input: EventHandler<String>,
    on_confirm_blur: EventHandler<()>,
    on_submit: EventHandler<FormEvent>,
) -> Element {
    rsx! {
        form {
            class: "auth-form",
            action: "javascript:void(0);",
            onsubmit: move |evt| on_submit.call(evt),
            div { class: "form-group",
                label { r#for: "register-name", "Full Name" }
                input {
                    id: "register-name",
                    r#type: "text",
                    placeholder: "Jane Doe",
                    required: true,
                    value: "{name}",
                    oninput: move |e| on_name_input.call(e.value()),
                    disabled: is_submitting,
                }
            }

            div { class: "form-group",
                label { r#for: "register-email", "Email address" }
                input {
                    id: "register-email",
                    r#type: "email",
                    class: if email_is_invalid { "input-invalid" },
                    placeholder: "jane@example.com",
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
                label { r#for: "register-password", "Password" }
                input {
                    id: "register-password",
                    r#type: "password",
                    placeholder: "•••••••• (min 6 chars)",
                    required: true,
                    value: "{password}",
                    oninput: move |e| on_password_input.call(e.value()),
                    disabled: is_submitting,
                }
            }

            div { class: "form-group",
                label { r#for: "register-confirm-password", "Confirm Password" }
                input {
                    id: "register-confirm-password",
                    r#type: "password",
                    class: if passwords_mismatch { "input-invalid" },
                    placeholder: "••••••••",
                    required: true,
                    value: "{confirm_password}",
                    oninput: move |e| on_confirm_input.call(e.value()),
                    onblur: move |_| on_confirm_blur.call(()),
                    disabled: is_submitting,
                }
                span {
                    class: "field-hint-error",
                    style: if !passwords_mismatch { "visibility: hidden;" },
                    "Passwords do not match."
                }
            }

            button {
                r#type: "submit",
                class: "btn-auth-submit",
                disabled: is_submitting,
                if is_submitting { "Creating account..." } else { "Create Account" }
            }
        }
    }
}

/// Footer with link to the login page for existing users.
#[component]
fn RegisterFooter() -> Element {
    rsx! {
        div { class: "auth-footer",
            p {
                "Already have an account? "
                Link { to: Route::LoginPage {}, class: "auth-link", "Log in here" }
            }
        }
    }
}
