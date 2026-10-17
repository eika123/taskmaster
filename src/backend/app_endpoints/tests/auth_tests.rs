use crate::backend::app_endpoints::{login, register};

#[tokio::test]
async fn test_email_validation_in_registration_and_login() {
    // 1. Invalid email formats rejected at registration
    let bad_emails = vec![
        "plainaddress",
        "#@%^%#$@#$@#.com",
        "@example.com",
        "Joe Smith <email@example.com>",
        "email.example.com",
        "email@example@example.com",
        "evan@example",
        "evan@example.",
        "evan@example.c",
    ];

    for bad in bad_emails {
        let res = register("Test User".to_string(), bad.to_string(), "password123".to_string()).await;
        assert!(res.is_err(), "Expected email '{bad}' to be rejected");
        let err_msg = res.unwrap_err().to_string();
        assert!(err_msg.contains("Valid email is required"), "Unexpected error: {err_msg}");
    }

    // 2. Invalid email format rejected at login before hitting database
    let login_res = login("notanemail".to_string(), "anypassword".to_string()).await;
    assert!(login_res.is_err());
    assert!(login_res.unwrap_err().to_string().contains("Valid email is required"));

    // 3. Valid email format is accepted
    assert!(crate::types::is_valid_email("evan@example.com"));
    assert!(crate::types::is_valid_email("jane.doe@sub.domain.org"));
}

