//! # Backend Test Suite & Campaign Catalog
//!
//! This module groups together the backend test campaigns for the FlowTask application:
//!
//! - [`auth_tests`]: Input sanitation, RFC email validation, and authentication fail-fast boundaries.
//! - [`guardrails_tests`]: PostgreSQL triggers preventing orphaning of leaders/governors and accidental deletions.
//! - [`projects_tests`]: Project leadership, discussion thread clustered keys, and invitation workflows.
//! - [`tasks_tests`]: Task-to-project relational constraints, denormalized metadata, and cascade deletions.
//! - [`organizations_tests`]: Organization role seeding, built-in role immutability, and member invitation cycles.
//! - [`org_project_access_tests`]: Enterprise RBAC, multi-role potato people, additive grant ledgers, privilege ceilings,
//!   dynamic demotions, task assignment permissions, and Zero-Knowledge Need-to-Know data isolation.

mod auth_tests;
mod guardrails_tests;
mod database_foundations_tests;
mod workflow_tests;
mod org_project_access_tests;
mod organizations_tests;
mod projects_tests;
mod tasks_tests;
pub mod test_helpers;
