//! Local data: CLI home (`~/.grok`) + App-owned session index.

pub mod active;
pub mod app_index;
pub mod cli_config;
pub mod install;
pub mod sessions;

pub use active::live_session_ids;
pub use app_index::{
    archive_session, delete_from_app, import_cli_session, list_active_sessions,
    list_archived_sessions, list_cli_import_candidates, rename_in_index, restore_session,
    sync_record_from_disk, touch_session, SessionOrigin,
};
pub use cli_config::CliTomlConfig;
pub use sessions::{
    delete_session, group_sessions_by_project, load_session_timeline, normalize_project_key,
    project_display_name, rename_session, scan_sessions, LocalSession, ProjectGroup,
};
