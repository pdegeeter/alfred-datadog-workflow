//! Core library for the `alfred-dd` Alfred workflow.
//!
//! The binary is a thin dispatcher over these modules. Everything that can be
//! made pure (parsing, ranking, URL building, clipboard detection, item
//! building) lives here and is unit tested; the only impure boundary is
//! [`runner::CommandRunner`], which is mocked in tests. The workflow is
//! read-only: it never writes to Datadog.

pub mod action;
pub mod alfred;
pub mod app;
pub mod cache;
pub mod config;
pub mod dashboards;
pub mod filters;
pub mod frecency;
pub mod notebooks;
pub mod percent;
pub mod pup;
pub mod query;
pub mod ranking;
pub mod runner;
pub mod sources;
pub mod time;
pub mod urls;
