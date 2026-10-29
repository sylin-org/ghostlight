//! The install tree: everything that puts Ghostlight on a computer, keeps it current and takes it
//! off again.
//!
//! [`deployment`] installs a payload into the home's `bin` and [`migration`] carries a 1.x
//! installation over; [`reconcile`] then points every registration at `bin`: the browsers'
//! native hosts ([`native_host`]), the agents' MCP configurations ([`harness`]), the command
//! entry ([`command_path`]) and the platform entries ([`desktop_entry`], `windows_entry`). The
//! entry rule ([`entry`]) decides what any `ghostlight` process does first. [`icons`] and [`apps`]
//! find where each catalog agent's app is installed, for the window and for provenance. The
//! shared mechanics live in their own modules: the home's names ([`layout`]), files ([`files`]),
//! paths ([`paths`]), processes ([`processes`]) and the user's directories.

#[cfg(test)]
mod agent_list;
pub mod apps;
pub mod browser_package;
pub mod browser_profiles;
pub mod command_path;
pub mod container;
pub mod deployment;
pub mod desktop_entry;
pub mod entry;
pub mod files;
pub mod harness;
pub mod icons;
pub mod last_browser;
pub mod layout;
pub mod migration;
pub mod native_host;
mod packaged;
pub mod paths;
pub mod processes;
pub mod reconcile;
pub mod record;
pub mod setup_state;
pub mod swap;
pub mod update;
pub mod user_assets;
mod user_dirs;
pub mod user_path;
pub mod version_directories;
#[cfg(windows)]
pub mod windows_entry;
#[cfg(test)]
mod writer_golden;

pub use harness::*;

#[cfg(test)]
use harness::{
    definition, harness_roots, manual_json_setup, manual_setup, ConfigDialect, HarnessContext,
    HarnessRow, JsonDialect, PrivateView, CLAUDE_CONFIG_VARIABLE, CODEX_HOME_VARIABLE,
    CRUSH_CONFIG_VARIABLE, ROSTER, SERVER_NAME,
};
#[cfg(test)]
use layout::program_file_name;
#[cfg(test)]
use packaged::Packages;
