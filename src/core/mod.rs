//! # Core - Backend logic
//!
//! Reusable backend: shared state, pinger threads, config persistence,
//! export/import, and JSON serialization types.
//! This module has no UI dependencies and could be consumed by any frontend.

pub mod config;
pub mod export;
pub mod import;
pub mod json_types;
pub mod pinger;
pub mod state;
