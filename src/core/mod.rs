//! # Core - Backend logic
//!
//! Reusable backend: shared state, pinger threads, config persistence,
//! export/import, and data models.
//! This module has no UI dependencies and could be consumed by any frontend.

pub mod config;
pub mod export;
pub mod import;
pub mod modem_health;
pub mod models;
pub mod pinger;
pub mod preset_packs;
pub mod server_check;

// Re-export state types at the old path for convenience
pub use models::state;
pub use models::json_types;
