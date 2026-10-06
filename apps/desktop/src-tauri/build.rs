// SPDX-License-Identifier: Apache-2.0
//! Build the Tauri context and explicitly scope application commands.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "app_info",
            "result_window",
            "spike_finish",
            "spike_start",
            "spike_progress",
        ]),
    ))?;
    Ok(())
}
