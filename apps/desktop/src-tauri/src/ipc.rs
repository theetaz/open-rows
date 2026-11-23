// SPDX-License-Identifier: Apache-2.0
//! The generated IPC contract: every command, event and shared type.
#[cfg(test)]
mod tests;

use crate::connections::{
    self, attach, catalog, connection, credentials, diagnostics, exchange, files, inspection,
    monitor, processes, profiles, prompts, reveal, sessions,
};
use crate::{
    ai, ai_sharing, assistant, commands, diagram, dump, exports, imports, locations, mcp,
    mcp_client, menu, native_files, observability, projects, queries, report_save, results,
    run_file, security, settings, settings_watch, spike, tables, updates, windows, workspace,
};

#[expect(
    clippy::too_many_lines,
    reason = "the registry is one list of every command, which only grows"
)]
pub(crate) fn command_builder() -> tauri_specta::Builder<tauri::Wry> {
    let builder = tauri_specta::Builder::new().commands(tauri_specta::collect_commands![
        commands::app_info,
        ai::ai_key_prompt,
        ai::ai_key_delete,
        ai::ai_verify,
        ai::ai_cancel,
        ai::codex::codex_login,
        assistant::investigation_audit,
        assistant::investigation_schedule,
        assistant::investigation_schedule_cancel,
        assistant::investigation_schedule_pause,
        assistant::investigation_schedule_run,
        assistant::investigation_schedules,
        assistant::assistant_run,
        assistant::assistant_cancel,
        assistant::assistant_preview,
        assistant::assistant_explain,
        ai_sharing::ai_sharing_get,
        ai_sharing::ai_sharing_save,
        ai_sharing::ai_context_preview,
        ai_sharing::ai_audit_list,
        ai_sharing::ai_audit_purge,
        mcp_client::mcp_servers_list,
        mcp_client::projects::mcp_project_servers,
        mcp_client::projects::mcp_project_trust,
        mcp_client::projects::mcp_project_revoke,
        mcp_client::projects::mcp_project_tools,
        mcp_client::projects::mcp_project_secret,
        mcp_client::mcp_servers_save,
        mcp_client::mcp_server_secrets,
        mcp_client::mcp_server_delete,
        mcp_client::mcp_server_tools,
        mcp_client::mcp_tool_confirm,
        mcp::mcp_status,
        mcp::mcp_token_create,
        mcp::mcp_token_revoke,
        mcp::mcp_client_disconnect,
        mcp::apps::agent_apps,
        mcp::apps::agent_app_check,
        mcp::apps::agent_app_plan,
        mcp::apps::agent_app_test,
        mcp::apps::agent_activity,
        mcp::apps_connect::agent_app_connect,
        mcp::apps_connect::agent_app_disconnect,
        mcp::apps_connect::agent_app_renew,
        mcp::proposals::mcp_proposals_take,
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
        monitor::monitor_investigate,
        monitor::monitor_advice,
        monitor::monitor_propose,
        monitor::monitor_explain,
        monitor::monitor_hypopg,
        monitor::monitor_start,
        monitor::monitor_stop,
        monitor::monitor_pause,
        monitor::monitor_capabilities,
        monitor::monitor_snapshot,
        monitor::monitor_history,
        monitor::monitor_history_configure,
        monitor::monitor_history_purge,
        monitor::monitor_incidents,
        monitor::monitor_background,
        monitor::boards_list,
        monitor::board_save,
        monitor::board_import,
        monitor::board_export,
        monitor::board_approve,
        monitor::board_delete,
        monitor::board_run,
        queries::commands::query_execute,
        queries::resolve::query_resolve_source,
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
        projects::sharing::project_share_connections,
        projects::sql_items::project_sql_items,
        projects::sql_items::project_create_snippet,
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
        windows::commands::window_new,
        windows::commands::window_list,
        windows::commands::window_transfer,
        windows::commands::window_transfer_take,
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
        report_save::report_save,
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
        updates::update_check,
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
            monitor::MonitorChanged,
            monitor::MonitorIncident,
            settings_watch::SettingsChanged,
            mcp::McpActivity
        ])
        .constant(
            "DEFAULT_CUSTOM_ENV_COLOR",
            openrows_store::profile::EnvTag::new(openrows_store::profile::EnvKind::Custom).color,
        )
        .typ::<openrows_ai::AiSettings>()
        .typ::<monitor::MonitorWindow>()
        .typ::<openrows_mcp::McpSettings>()
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
