//! # Milestone M0: Foundations Test Campaign (TC0)
//!
//! Validates:
//! 1. `TIMESTAMPTZ` Schema Migration: ensures all temporal columns use PostgreSQL `timestamp with time zone`.
//! 2. `has_perm` SQL Function: tests bitwise mask validation in PostgreSQL.
//! 3. Strongly-Typed Rust `Permission` Bitflags: tests bitwise operations, default masks, and invariant helpers.
//! 4. Organization Role Default Bitmasks: verifies auto-seeded `'admin'` (1023) and `'member'` (1551) permissions masks.
//! 5. `DateTime<Utc>` serialization and round-trip via sqlx and Dioxus domain types.

use chrono::{DateTime, Utc};
use crate::backend::database::get_db;
use crate::types::{has_perm, Permission};

#[tokio::test]
async fn test_timestamptz_columns_in_information_schema() {
    let pool = get_db().await;

    let target_columns = [
        ("organization", "created_at"),
        ("organization_members", "joined_at"),
        ("project", "created_at"),
        ("project_members", "joined_at"),
        ("task_message", "created_at"),
        ("user_inbox_message", "created_at"),
        ("organization_invitation", "created_at"),
        ("project_invitation", "created_at"),
        ("session", "expires_at"),
    ];

    for (table, col) in target_columns {
        let data_type: String = sqlx::query_scalar(
            "SELECT data_type FROM information_schema.columns WHERE table_schema = CURRENT_SCHEMA AND table_name = $1 AND column_name = $2",
        )
        .bind(table)
        .bind(col)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|e| panic!("Failed checking data_type for {table}.{col}: {e}"));

        assert_eq!(
            data_type, "timestamp with time zone",
            "Expected {table}.{col} to be 'timestamp with time zone', got '{data_type}'"
        );
    }
}

#[tokio::test]
async fn test_has_perm_sql_function() {
    let pool = get_db().await;

    // Test cases: (mask, required, expected)
    let cases = [
        (15i64, 4i64, true),            // EXECUTE in LEGACY_MEMBER (15)
        (15i64, 32i64, false),          // ADMIN in LEGACY_MEMBER (15)
        (0i64, 1i64, false),            // Any perm in 0
        (1023i64, 512i64, true),        // MESSAGE_MEMBERS in ADMIN_DEFAULT (1023)
        (1023i64, 1024i64, false),      // IGNORABLE_BY_MEMBERS not in ADMIN_DEFAULT
        (1551i64, 1024i64, true),       // IGNORABLE_BY_MEMBERS in MEMBER_DEFAULT (1551)
        (1551i64, 4i64, true),          // EXECUTE in MEMBER_DEFAULT
        (1551i64, 32i64, false),         // ADMIN not in MEMBER_DEFAULT
    ];

    for (mask, required, expected) in cases {
        let result: bool = sqlx::query_scalar("SELECT has_perm($1::BIGINT, $2::BIGINT)")
            .bind(mask)
            .bind(required)
            .fetch_one(pool)
            .await
            .unwrap_or_else(|e| panic!("has_perm({mask}, {required}) failed: {e}"));

        assert_eq!(
            result, expected,
            "has_perm({mask}, {required}) returned {result}, expected {expected}"
        );
    }

    // Verify NULL safety
    let null_left: bool = sqlx::query_scalar("SELECT has_perm(NULL, 1::BIGINT)")
        .fetch_one(pool)
        .await
        .unwrap();
    assert!(!null_left, "has_perm(NULL, 1) should be false");

    let null_right: bool = sqlx::query_scalar("SELECT has_perm(15::BIGINT, NULL)")
        .fetch_one(pool)
        .await
        .unwrap();
    assert!(!null_right, "has_perm(15, NULL) should be false");
}

#[test]
fn test_rust_permission_bitflags_invariants() {
    // Check bit assignments match review spec
    assert_eq!(Permission::READ.as_i64(), 1);
    assert_eq!(Permission::SUGGEST.as_i64(), 2);
    assert_eq!(Permission::EXECUTE.as_i64(), 4);
    assert_eq!(Permission::ASSIGN.as_i64(), 8);
    assert_eq!(Permission::MANAGE_MEMBERS.as_i64(), 16);
    assert_eq!(Permission::ADMIN.as_i64(), 32);
    assert_eq!(Permission::MANAGE_WORKFLOW.as_i64(), 64);
    assert_eq!(Permission::MODERATE_DISCUSSIONS.as_i64(), 128);
    assert_eq!(Permission::POST_ANNOUNCEMENTS.as_i64(), 256);
    assert_eq!(Permission::MESSAGE_MEMBERS.as_i64(), 512);
    assert_eq!(Permission::IGNORABLE_BY_MEMBERS.as_i64(), 1024);

    // Preset composite masks
    assert_eq!(Permission::LEGACY_MEMBER.as_i64(), 15);
    assert_eq!(Permission::LEGACY_ADMIN.as_i64(), 63);
    assert_eq!(Permission::ADMIN_DEFAULT.as_i64(), 1023);
    assert_eq!(Permission::MEMBER_DEFAULT.as_i64(), 1551);

    // Bitwise operators
    let combo = Permission::READ | Permission::EXECUTE;
    assert_eq!(combo.as_i64(), 5);
    assert!(combo.contains(Permission::READ));
    assert!(combo.contains(Permission::EXECUTE));
    assert!(!combo.contains(Permission::SUGGEST));

    // Intersects
    assert!(combo.intersects(Permission::READ | Permission::SUGGEST));
    assert!(!combo.intersects(Permission::SUGGEST | Permission::ADMIN));

    // has_perm helper
    assert!(has_perm(Permission::MEMBER_DEFAULT.as_i64(), Permission::EXECUTE));
    assert!(has_perm(Permission::MEMBER_DEFAULT.as_i64(), Permission::IGNORABLE_BY_MEMBERS));
    assert!(!has_perm(Permission::MEMBER_DEFAULT.as_i64(), Permission::ADMIN));
    assert!(has_perm(Permission::ADMIN_DEFAULT.as_i64(), Permission::ADMIN));
    assert!(!has_perm(Permission::ADMIN_DEFAULT.as_i64(), Permission::IGNORABLE_BY_MEMBERS));
}

#[tokio::test]
async fn test_organization_role_seeded_permissions_masks() {
    let pool = get_db().await;
    let unique_timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    // Create an organization (governor user 1)
    let org_id: i64 = sqlx::query_scalar(
        "INSERT INTO organization (name, slug, governing_user_id) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind("TC0 Permissions Test Org")
    .bind(format!("tc0-perms-org-{unique_timestamp}"))
    .bind(1)
    .fetch_one(pool)
    .await
    .unwrap();

    // Query seeded permissions_mask
    let roles: Vec<(String, i64)> = sqlx::query_as(
        "SELECT role, permissions_mask FROM organization_role WHERE organization_id = $1 ORDER BY role ASC",
    )
    .bind(org_id)
    .fetch_all(pool)
    .await
    .unwrap();

    let admin_mask = roles.iter().find(|(r, _)| r == "admin").map(|(_, m)| *m);
    let member_mask = roles.iter().find(|(r, _)| r == "member").map(|(_, m)| *m);

    assert_eq!(
        admin_mask,
        Some(Permission::ADMIN_DEFAULT.as_i64()),
        "Admin role permissions_mask must match ADMIN_DEFAULT (1023)"
    );
    assert_eq!(
        member_mask,
        Some(Permission::MEMBER_DEFAULT.as_i64()),
        "Member role permissions_mask must match MEMBER_DEFAULT (1551)"
    );
}

#[tokio::test]
async fn test_timestamptz_roundtrip_precision() {
    let pool = get_db().await;
    let unique_timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    let before = Utc::now();

    let (org_id, created_at): (i64, DateTime<Utc>) = sqlx::query_as(
        "INSERT INTO organization (name, slug, governing_user_id) VALUES ($1, $2, $3) RETURNING id, created_at",
    )
    .bind("TC0 Timestamp Roundtrip Org")
    .bind(format!("tc0-timestamp-org-{unique_timestamp}"))
    .bind(1)
    .fetch_one(pool)
    .await
    .unwrap();

    let after = Utc::now();

    assert!(
        created_at >= before - chrono::Duration::seconds(2),
        "created_at ({created_at:?}) was before test start ({before:?})"
    );
    assert!(
        created_at <= after + chrono::Duration::seconds(2),
        "created_at ({created_at:?}) was after test end ({after:?})"
    );

    // Read back via SELECT
    let fetched_created_at: DateTime<Utc> = sqlx::query_scalar(
        "SELECT created_at FROM organization WHERE id = $1",
    )
    .bind(org_id)
    .fetch_one(pool)
    .await
    .unwrap();

    assert_eq!(created_at, fetched_created_at, "TIMESTAMPTZ round-trip must be identical");
}

