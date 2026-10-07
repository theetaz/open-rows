// SPDX-License-Identifier: Apache-2.0
//! `OpenRows` native shell and its generated IPC contract.
mod commands;
mod editor_report;
mod grid_report;
#[cfg(target_os = "macos")]
mod material;
mod results;
mod spike;
mod startup;
mod startup_report;

fn command_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::new()
        .commands(tauri_specta::collect_commands![
            commands::app_info,
            spike::spike_finish,
            spike::spike_start,
            spike::spike_progress
        ])
        .typ::<results::WindowRequest>()
        .typ::<results::ResultError>()
}

/// Export the same command registry used by the running application.
///
/// # Errors
/// Returns an exporter error if the output cannot be generated or written.
pub fn export_bindings(path: &std::path::Path) -> Result<(), specta_typescript::Error> {
    command_builder().export(
        specta_typescript::Typescript::default().header("// SPDX-License-Identifier: Apache-2.0"),
        path,
    )
}

/// Run the native application event loop.
///
/// # Errors
/// Returns a Tauri error if setup or the event loop fails.
pub fn run() -> Result<(), tauri::Error> {
    startup_report::initialize();
    startup_report::record(startup_report::Phase::BuilderStart);
    let builder = spike::configure(tauri::Builder::default())?;
    let controls = command_builder().invoke_handler();
    let binary: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool =
        tauri::generate_handler![results::result_window];
    startup_report::instrument(builder)
        .invoke_handler(move |invoke| {
            if invoke.message.command() == "result_window" {
                binary(invoke)
            } else {
                controls(invoke)
            }
        })
        .setup(startup::install)
        .run(tauri::generate_context!())
}

#[cfg(test)]
mod tests {
    #[test]
    fn bindings_are_fresh() -> Result<(), Box<dyn std::error::Error>> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let temporary = root.join(format!(".bindings-check-{}.ts", std::process::id()));
        let generated: Result<String, Box<dyn std::error::Error>> = (|| {
            super::export_bindings(&temporary)?;
            std::fs::read_to_string(&temporary).map_err(Into::into)
        })();
        if temporary.exists() {
            std::fs::remove_file(&temporary)?;
        }
        let expected =
            std::fs::read_to_string(root.join("../../../packages/ipc/src/generated/bindings.ts"))?;
        let actual: String = generated?;
        assert_eq!(actual, expected, "IPC bindings are stale; run just gen-ipc");
        Ok(())
    }
}
