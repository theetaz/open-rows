// SPDX-License-Identifier: Apache-2.0
//! `OpenRows` native shell and its generated IPC contract.
mod ai;
mod ai_sharing;
mod assistant;
#[cfg(all(feature = "test-automation", debug_assertions, unix))]
mod automation;
pub mod cli;
mod commands;
mod connections;
mod diagram;
mod driver_host;
mod dump;
mod editor_report;
mod exports;
mod grid_report;
mod imports;
mod ipc;
mod jobs;
mod links;
mod locations;
#[cfg(target_os = "macos")]
mod material;
mod mcp;
mod mcp_client;
mod menu;
mod native_files;
mod observability;
mod projects;
mod queries;
mod report_save;
mod result_store;
mod results;
mod run_file;
mod runtime;
mod security;
mod settings;
mod settings_watch;
mod spike;
mod startup;
mod startup_report;
mod tables;
mod titlebar;
mod updates;
mod window_frame;
mod windows;
mod workspace;

pub use ipc::export_bindings;
pub use runtime::run;
