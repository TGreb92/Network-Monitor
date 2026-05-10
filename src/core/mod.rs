//! # Core — Backend logic
//!
//! Reusable backend: shared state, pinger threads, config persistence, and export.
//! This module has no UI dependencies and could be consumed by any frontend.

pub mod config;
pub mod export;
pub mod pinger;
pub mod state;
