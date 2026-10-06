// SPDX-License-Identifier: Apache-2.0
//! `OpenRows` desktop entry point.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> Result<(), tauri::Error> {
    openrows_app::run()
}
