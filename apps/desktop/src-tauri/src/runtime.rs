// SPDX-License-Identifier: Apache-2.0
//! The application's lifecycle: builder, IPC routing, setup and exit.
#[cfg(all(feature = "test-automation", debug_assertions, unix))]
use crate::automation;
use crate::{
    ai, ai_sharing, assistant, connections, driver_host, dump, exports, imports, ipc, locations,
    mcp, mcp_client, menu, native_files, observability, queries, result_store, results, run_file,
    security, settings, settings_watch, startup, startup_report, window_frame, windows,
};
use connections::monitor;
use tauri::Manager;
use tauri::ipc::Invoke;

/// Run the native application event loop.
///
/// # Errors
/// Returns a Tauri error if setup or the event loop fails.
pub fn run() -> Result<(), tauri::Error> {
    observability::install_panic_hook();
    startup_report::initialize();
    startup_report::record(startup_report::Phase::BuilderStart);
    let builder = managed_builder();
    let specta = ipc::command_builder();
    let controls = specta.invoke_handler();
    let binary: fn(Invoke<tauri::Wry>) -> bool = tauri::generate_handler![
        results::result_window,
        results::result_cell_value,
        monitor::monitor_window,
        monitor::monitor_top_statements
    ];
    startup_report::instrument(builder)
        .invoke_handler(move |invoke| route_invoke(invoke, binary, &controls))
        .setup(move |app| install(app, &specta))
        .build(application_context())?
        .run(on_run_event);
    Ok(())
}

/// The plugins and the state that exists before setup runs.
fn managed_builder() -> tauri::Builder<tauri::Wry> {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(window_frame::plugin())
        .manage(native_files::NativeFiles::default())
        .manage(security::Security::default())
        .manage(queries::Queries::default())
        .manage(imports::Imports::default())
        .manage(exports::pins::ResultPins::default())
        .manage(exports::outputs::ExportOutputs::default())
        .manage(run_file::RunFiles::default())
        .manage(dump::DumpJobs::default())
}

/// Admit a call through the app lock, then send it to the handler that owns it.
fn route_invoke(
    invoke: Invoke<tauri::Wry>,
    binary: fn(Invoke<tauri::Wry>) -> bool,
    controls: &impl Fn(Invoke<tauri::Wry>) -> bool,
) -> bool {
    #[cfg(all(feature = "test-automation", debug_assertions, unix))]
    if matches!(
        invoke.message.command(),
        "automation_reply" | "automation_observations"
    ) {
        return automation::handler(invoke);
    }
    if let Err(error) = invoke
        .message
        .webview()
        .state::<security::Security>()
        .admit(invoke.message.command())
    {
        invoke
            .message
            .webview()
            .state::<connections::Connections>()
            .monitors
            .stop_all();
        invoke.resolver.reject(error);
        return true;
    }
    if matches!(
        invoke.message.command(),
        "result_window" | "result_cell_value" | "monitor_window" | "monitor_top_statements"
    ) {
        binary(invoke)
    } else {
        controls(invoke)
    }
}

fn install(
    app: &mut tauri::App,
    specta: &tauri_specta::Builder<tauri::Wry>,
) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(all(feature = "test-automation", debug_assertions, unix))]
    automation::install(app)?;
    locations::install(app)?;
    observability::install(app)?;
    specta.mount_events(app);
    app.manage(ai::AiState::default());
    app.manage(assistant::AssistantState::default());
    mcp_client::install(app)?;
    app.manage(assistant::ScheduledState::default());
    settings::install(app);
    result_store::install(app)?;
    driver_host::install(app)?;
    connections::install(app)?;
    ai_sharing::install(app)?;
    mcp::install(app)?;
    native_files::install(app);
    settings::apply_saved(app);
    if let Some(value) = settings::current(&app.state::<settings::SettingsFile>()) {
        app.state::<security::Security>().configure(&value, true);
    }
    security::monitor(app.handle());
    settings_watch::watch(app.handle());
    menu::install(app)?;
    startup::install(app)?;
    windows::install(app)
}

/// The test webview has a separate storage directory before it is created.
fn application_context() -> tauri::Context<tauri::Wry> {
    #[cfg_attr(
        not(all(feature = "test-automation", debug_assertions, unix)),
        expect(unused_mut, reason = "only test builds configure an isolated webview")
    )]
    let mut context = tauri::generate_context!();
    #[cfg(all(feature = "test-automation", debug_assertions, unix))]
    if let Some(root) = std::env::var_os("OPENROWS_E2E_ROOT") {
        for window in &mut context.config_mut().app.windows {
            window.data_directory = Some(std::path::PathBuf::from(&root).join("webview"));
            window.incognito = true;
        }
    }
    context
}

/// Route opened files to the native-file queue and release resources on exit.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri hands each RunEvent over by value"
)]
fn on_run_event(app: &tauri::AppHandle, event: tauri::RunEvent) {
    #[cfg(target_os = "macos")]
    if let tauri::RunEvent::Opened { urls } = &event {
        native_files::opened(app, urls);
    }
    windows::on_run_event(app, &event);
    if let tauri::RunEvent::Exit = event {
        mcp::exit(app);
        app.state::<connections::Connections>().monitors.stop_all();
        connections::helpers::exit(app);
        result_store::exit(app);
        observability::exit(app);
    }
}
