// SPDX-License-Identifier: Apache-2.0
//! `OpenRows` native shell and its generated IPC contract.
mod commands;
mod connections;
mod diagram;
mod driver_host;
mod dump;
mod editor_report;
mod exports;
mod grid_report;
mod imports;
mod jobs;
mod locations;
#[cfg(target_os = "macos")]
mod material;
mod menu;
mod native_files;
mod observability;
mod projects;
mod queries;
mod result_store;
mod results;
mod run_file;
mod security;
mod settings;
mod settings_watch;
mod spike;
mod startup;
mod startup_report;
mod tables;
mod titlebar;
mod window_frame;
mod workspace;

use connections::{
    attach, catalog, connection, credentials, diagnostics, exchange, files, inspection, processes,
    profiles, prompts, reveal, sessions,
};
use tauri::Manager;

#[expect(
    clippy::too_many_lines,
    reason = "the registry is one list of every command, which only grows"
)]
fn command_builder() -> tauri_specta::Builder<tauri::Wry> {
    let builder = tauri_specta::Builder::new().commands(tauri_specta::collect_commands![
        commands::app_info,
        security::security_status,
        security::security_lock,
        security::security_unlock,
        security::security_activity,
        spike::spike_finish,
        spike::spike_start,
        spike::spike_progress,
        profiles::profiles_list,
        profiles::profiles_search,
        exchange::connections_export,
        exchange::connections_import_preview,
        exchange::connections_import_apply,
        exchange::connections_import_cancel,
        credentials::credential_test,
        credentials::credential_cancel,
        profiles::profile_get,
        profiles::profile_create,
        profiles::profile_update,
        profiles::profile_duplicate,
        profiles::profile_delete,
        profiles::profile_parse_url,
        reveal::profile_reveal_secret,
        diagnostics::connection_test,
        files::connection_pick_file,
        attach::connection_attach_database,
        attach::connection_detach_database,
        prompts::ssh_prompt_answer,
        connection::connection_open,
        connection::connection_close,
        sessions::session_open,
        sessions::session_close,
        catalog::catalog_databases,
        catalog::catalog_schemas,
        catalog::catalog_objects,
        inspection::catalog_capabilities,
        inspection::catalog_object_detail,
        inspection::catalog_extensions,
        inspection::catalog_roles,
        inspection::catalog_role_grants,
        catalog::catalog_search,
        catalog::catalog_invalidate,
        catalog::catalog_check,
        processes::processes_list,
        processes::process_signal,
        queries::commands::query_execute,
        queries::commands::query_cancel,
        queries::explain::query_explain,
        queries::commands::results_release,
        queries::commands::history_search,
        queries::commands::history_delete,
        queries::commands::history_clear,
        queries::history_privacy::history_literals,
        results::result_cell_info,
        queries::saved::saved_queries_list,
        queries::saved::saved_query_create,
        queries::saved::saved_query_rename,
        queries::saved::saved_query_delete,
        queries::files::sql_file_save_as,
        queries::files::sql_file_save,
        queries::files::sql_file_open,
        queries::file_watch::sql_file_stat,
        queries::file_watch::sql_file_read,
        projects::commands::project_choose,
        projects::commands::project_load,
        projects::commands::project_set_default_connection,
        projects::commands::project_create_folder,
        projects::commands::project_create_query,
        projects::commands::project_rename,
        projects::commands::project_move,
        projects::commands::project_duplicate,
        projects::commands::project_delete,
        projects::commands::project_reveal,
        workspace::workspace_state_read,
        workspace::workspace_state_write,
        tables::commands::table_structure,
        tables::commands::table_sql,
        tables::commands::table_page,
        tables::commands::table_count,
        tables::edit_commands::table_edit_info,
        tables::snapshot_commands::table_edit_row,
        tables::snapshot_commands::table_edit_rows,
        tables::edit_commands::table_edit,
        tables::recovery_commands::table_edits_save,
        tables::recovery_commands::table_edits_saved,
        tables::recovery_commands::table_edits_clear,
        tables::file_value::table_edit_file,
        tables::schema_commands::table_detail,
        tables::schema_commands::table_alter,
        tables::alter_checks::table_alter_preview,
        tables::object_commands::object_change,
        tables::object_commands::routine_definition,
        exports::commands::export_start,
        imports::commands::import_pick_file,
        imports::commands::import_preview,
        imports::commands::import_start,
        imports::commands::import_cancel,
        exports::outputs::export_reveal,
        exports::outputs::export_open,
        run_file::commands::run_file_pick,
        run_file::commands::run_file_check,
        run_file::commands::run_file_start,
        diagram::commands::schema_diagram,
        diagram::commands::diagram_save,
        dump::commands::dump_tools,
        dump::commands::dump_context,
        dump::commands::dump_backup_preview,
        dump::commands::dump_backup_pick,
        dump::commands::dump_backup_start,
        dump::commands::dump_cancel,
        dump::restore::dump_restore_pick,
        dump::restore::dump_restore_contents,
        dump::restore::dump_restore_check,
        dump::restore::dump_restore_start,
        native_files::native_files_take,
        native_files::database_reveal,
        locations::data_locations,
        locations::data_location_choose,
        locations::data_location_apply,
        settings::settings_read,
        settings::settings_write,
        menu::menu_sync,
        observability::commands::crash_recovery_status,
        observability::commands::open_logs_folder,
        observability::commands::ui_report,
        tables::recovery_commands::table_edits_journaled
    ]);
    with_events_and_types(builder)
}

/// The events, constants and extra types the bindings carry besides commands.
fn with_events_and_types(
    builder: tauri_specta::Builder<tauri::Wry>,
) -> tauri_specta::Builder<tauri::Wry> {
    use connections::catalog_cache;
    builder
        .events(tauri_specta::collect_events![
            sessions::SessionStateChanged,
            catalog_cache::CatalogChanged,
            prompts::SshPromptChanged,
            menu::MenuCommand,
            settings_watch::SettingsChanged
        ])
        .constant(
            "DEFAULT_CUSTOM_ENV_COLOR",
            openrows_store::profile::EnvTag::new(openrows_store::profile::EnvKind::Custom).color,
        )
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
    )?;
    // Specta's documented enum unions include trailing spaces; keep generated diffs clean.
    let source = std::fs::read_to_string(path)?;
    let clean = source
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(path, format!("{clean}\n"))?;
    Ok(())
}

/// Run the native application event loop.
///
/// # Errors
/// Returns a Tauri error if setup or the event loop fails.
pub fn run() -> Result<(), tauri::Error> {
    observability::install_panic_hook();
    startup_report::initialize();
    startup_report::record(startup_report::Phase::BuilderStart);
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(window_frame::plugin())
        .manage(native_files::NativeFiles::default())
        .manage(security::Security::default())
        .manage(queries::Queries::default())
        .manage(imports::Imports::default())
        .manage(exports::pins::ResultPins::default())
        .manage(exports::outputs::ExportOutputs::default())
        .manage(run_file::RunFiles::default())
        .manage(dump::DumpJobs::default());
    let specta = command_builder();
    let controls = specta.invoke_handler();
    let binary: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool =
        tauri::generate_handler![results::result_window, results::result_cell_value];
    startup_report::instrument(builder)
        .invoke_handler(move |invoke| {
            if let Err(error) = invoke
                .message
                .webview()
                .state::<security::Security>()
                .admit(invoke.message.command())
            {
                invoke.resolver.reject(error);
                return true;
            }
            if matches!(
                invoke.message.command(),
                "result_window" | "result_cell_value"
            ) {
                binary(invoke)
            } else {
                controls(invoke)
            }
        })
        .setup(move |app| {
            locations::install(app)?;
            observability::install(app)?;
            specta.mount_events(app);
            settings::install(app);
            result_store::install(app)?;
            driver_host::install(app)?;
            connections::install(app)?;
            native_files::install(app);
            settings::apply_saved(app);
            if let Some(value) = settings::current(&app.state::<settings::SettingsFile>()) {
                app.state::<security::Security>().configure(&value, true);
            }
            security::monitor(app.handle());
            settings_watch::watch(app.handle());
            menu::install(app)?;
            startup::install(app)
        })
        .build(tauri::generate_context!())?
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Opened { urls } = &event {
                native_files::enqueue(app, urls.iter().filter_map(|url| url.to_file_path().ok()));
            }
            if let tauri::RunEvent::Exit = event {
                connections::helpers::exit(app);
                result_store::exit(app);
                observability::exit(app);
            }
        });
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn exported_commands_are_registered_and_allowed_in_the_main_window()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let bindings =
            std::fs::read_to_string(root.join("../../../packages/ipc/src/generated/bindings.ts"))?;
        let manifest = std::fs::read_to_string(root.join("build.rs"))?;
        let capability: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
            root.join("capabilities/main.json"),
        )?)?;
        let permissions = capability["permissions"]
            .as_array()
            .ok_or("Missing permissions")?;
        let mut checked = 0;
        for line in bindings
            .lines()
            .filter(|line| line.contains("__TAURI_INVOKE"))
        {
            let Some((_, rest)) = line.rsplit_once("(\"") else {
                continue;
            };
            let Some((command, _)) = rest.split_once('"') else {
                continue;
            };
            assert!(
                manifest.contains(&format!("\"{command}\"")),
                "Unregistered command: {command}"
            );
            let permission = format!("allow-{}", command.replace('_', "-"));
            assert!(
                permissions
                    .iter()
                    .any(|item| item.as_str() == Some(&permission)),
                "Missing permission: {permission}"
            );
            checked += 1;
        }
        assert!(checked >= 30, "No command contract was checked");
        Ok(())
    }
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
