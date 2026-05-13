//! # Tabs - one module per application tab
//!
//! Each tab has its own state struct and render function.

pub mod config;
pub mod console;
#[cfg(debug_assertions)]
pub mod debug;
pub mod help;
pub mod monitor;
