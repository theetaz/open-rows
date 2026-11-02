// SPDX-License-Identifier: Apache-2.0
//! `OpenRows` desktop entry point; `open <target>` forwards to the app (docs/16 §3.1).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> Result<std::process::ExitCode, tauri::Error> {
    if let Some(code) = openrows_app::cli::open_command(std::env::args_os().skip(1)) {
        return Ok(code);
    }
    openrows_app::run()?;
    Ok(std::process::ExitCode::SUCCESS)
}
