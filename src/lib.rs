//! Grok Desktop library crate — pure modules + UI for the binary and unit tests.
//!
//! The desktop app is an ACP client over the official `grok agent stdio` CLI.

pub mod acp;
pub mod app;
pub mod attachments;
pub mod config;
pub mod i18n;
pub mod local;
pub mod models_cache;
pub mod session_fsm;
pub mod session_store;
pub mod stream;
pub mod ui;
pub mod spawn_util;
pub mod update;
pub mod win_chrome;

pub use app::GrokApp;
pub use config::AppConfig;
