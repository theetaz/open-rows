// SPDX-License-Identifier: Apache-2.0
//! The native menu bar (FR-APP-14). Every custom item's id is a command id of
//! the webview's command registry: choosing it emits [`MenuCommand`] and the
//! webview runs the command, exactly as from the palette or a shortcut. The
//! webview owns shortcuts and enabled state and pushes them with
//! [`menu_sync`], so rebinding a shortcut updates the menu too. Standard
//! items (Edit, Window, Hide, Quit) are the platform's predefined items.
#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri deserialises command arguments into owned values"
)]
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::menu::{Menu, MenuItem, MenuItemBuilder, Submenu, SubmenuBuilder};
use tauri::{Manager, State, Wry};
use tauri_specta::Event as _;

/// One entry of a menu.
enum Entry {
    /// A command of the webview's registry: `(id, label)`.
    Command(&'static str, &'static str),
    Separator,
    Submenu(&'static str, &'static [Entry]),
    /// A platform item (About, Hide, Undo, Minimize…).
    Native(Native),
}

#[derive(Clone, Copy)]
enum Native {
    About,
    Services,
    Hide,
    HideOthers,
    ShowAll,
    Quit,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    Fullscreen,
    Minimize,
    Zoom,
}

use Entry::{Command as C, Native as N, Separator as S};

const APP: &[Entry] = &[
    N(Native::About),
    S,
    C("settings", "Settings…"),
    S,
    N(Native::Services),
    S,
    N(Native::Hide),
    N(Native::HideOthers),
    N(Native::ShowAll),
    S,
    N(Native::Quit),
];
const FILE: &[Entry] = &[
    C("new-connection", "New Connection…"),
    C("new-tab", "New Query Tab"),
    S,
    C("open-file", "Open SQL File…"),
    S,
    C("save", "Save"),
    C("save-as", "Save As…"),
    S,
    C("commit-changes", "Commit Changes…"),
    C("discard-changes", "Discard Changes"),
    S,
    C("export-table", "Export Table…"),
    C("export-results", "Export Results…"),
    C("import-data", "Import…"),
    S,
    C("close-tab", "Close Tab"),
];
const EDIT: &[Entry] = &[
    N(Native::Undo),
    N(Native::Redo),
    S,
    N(Native::Cut),
    N(Native::Copy),
    N(Native::Paste),
    N(Native::SelectAll),
    S,
    C("copy-selection", "Copy Rows"),
    Entry::Submenu("Copy As", COPY_AS),
    C("insert-row", "Insert Row"),
    C("delete-row", "Delete Rows"),
    S,
    C("focus-filter", "Filter Rows"),
];
const COPY_AS: &[Entry] = &[
    C("copy-as-csv", "CSV"),
    C("copy-as-json", "JSON"),
    C("copy-as-markdown", "Markdown"),
    C("copy-as-insert", "SQL INSERT"),
    C("copy-as-where-in", "WHERE IN"),
];
const GRID_ROWS: &[Entry] = &[
    C("grid-compact", "Compact"),
    C("grid-normal", "Normal"),
    C("grid-comfortable", "Comfortable"),
];
const APPEARANCE: &[Entry] = &[
    C("theme-light", "Light"),
    C("theme-dark", "Dark"),
    C("theme-system", "Use System Setting"),
];
const VIEW: &[Entry] = &[
    C("sidebar", "Toggle Sidebar"),
    C("inspector", "Toggle Inspector"),
    C("toggle-columns", "Toggle Columns Panel"),
    C("pin-column", "Pin or Unpin Column"),
    C("reset-columns", "Reset Column Layout"),
    S,
    Entry::Submenu("Appearance", APPEARANCE),
    Entry::Submenu("Grid Rows", GRID_ROWS),
    S,
    C("commands", "Command Palette…"),
    C("quick", "Quick Open…"),
    C("show-er-diagram", "Show ER Diagram"),
    S,
    N(Native::Fullscreen),
];
const QUERY: &[Entry] = &[
    C("run-current", "Run Current"),
    C("run-all", "Run All"),
    C("cancel", "Cancel Query"),
    S,
    C("beautify", "Beautify SQL"),
    C("uglify", "Uglify SQL"),
    C("insert-snippet", "Insert Snippet…"),
];
const CONNECTION: &[Entry] = &[
    C("connect", "Connect"),
    C("disconnect", "Disconnect"),
    S,
    C("database", "Switch Database…"),
    C("refresh", "Refresh"),
    S,
    C("processes", "Show Processes"),
    S,
    C("safe", "Safe Mode"),
];
const WINDOW: &[Entry] = &[N(Native::Minimize), N(Native::Zoom)];
const HELP: &[Entry] = &[
    C("help-search", "Search Commands…"),
    S,
    C("shortcuts", "Keyboard Shortcuts…"),
    S,
    C("open-logs", "Open Logs Folder"),
];

/// The menu bar, left to right.
const MENUS: &[(&str, &[Entry])] = &[
    ("OpenRows", APP),
    ("File", FILE),
    ("Edit", EDIT),
    ("View", VIEW),
    ("Query", QUERY),
    ("Connection", CONNECTION),
    ("Window", WINDOW),
    ("Help", HELP),
];

/// A menu command chosen by the user; `id` is the registry command id.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MenuCommand {
    id: String,
}

impl tauri_specta::Event for MenuCommand {
    const NAME: &'static str = "menu-command";
}

/// Enabled state and shortcut of one command item, from the webview.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MenuItemState {
    id: String,
    enabled: bool,
    /// Tauri accelerator such as `Cmd+Shift+Enter`; `null` for none.
    accelerator: Option<String>,
}

/// The command items of the installed menu, by id.
#[derive(Default)]
pub(crate) struct MenuItems(HashMap<String, MenuItem<Wry>>);

fn native(
    builder: SubmenuBuilder<'_, Wry, tauri::AppHandle>,
    item: Native,
) -> SubmenuBuilder<'_, Wry, tauri::AppHandle> {
    match item {
        Native::About => builder.about(None),
        Native::Services => builder.services(),
        Native::Hide => builder.hide(),
        Native::HideOthers => builder.hide_others(),
        Native::ShowAll => builder.show_all(),
        Native::Quit => builder.quit(),
        Native::Undo => builder.undo(),
        Native::Redo => builder.redo(),
        Native::Cut => builder.cut(),
        Native::Copy => builder.copy(),
        Native::Paste => builder.paste(),
        Native::SelectAll => builder.select_all(),
        Native::Fullscreen => builder.fullscreen(),
        Native::Minimize => builder.minimize(),
        Native::Zoom => builder.maximize(),
    }
}

fn submenu(
    app: &tauri::AppHandle,
    title: &str,
    entries: &[Entry],
    items: &mut MenuItems,
) -> tauri::Result<Submenu<Wry>> {
    let mut builder = SubmenuBuilder::new(app, title);
    for entry in entries {
        builder = match entry {
            Entry::Command(id, label) => {
                let item = MenuItemBuilder::with_id(*id, *label).build(app)?;
                items.0.insert((*id).to_owned(), item.clone());
                builder.item(&item)
            }
            Entry::Separator => builder.separator(),
            Entry::Submenu(label, nested) => builder.item(&submenu(app, label, nested, items)?),
            Entry::Native(item) => native(builder, *item),
        };
    }
    builder.build()
}

/// Build the menu bar; returns it with its command items.
fn build(app: &tauri::AppHandle) -> tauri::Result<(Menu<Wry>, MenuItems)> {
    let mut items = MenuItems::default();
    let menu = Menu::new(app)?;
    for (title, entries) in MENUS {
        let sub = submenu(app, title, entries, &mut items)?;
        #[cfg(target_os = "macos")]
        match *title {
            "Window" => sub.set_as_windows_menu_for_nsapp()?,
            "Help" => sub.set_as_help_menu_for_nsapp()?,
            _ => {}
        }
        menu.append(&sub)?;
    }
    Ok((menu, items))
}

/// Install the menu bar on macOS and forward its command items to the webview.
///
/// # Errors
/// A menu item cannot be created or the menu cannot be set.
pub(crate) fn install(app: &mut tauri::App) -> tauri::Result<()> {
    if !cfg!(target_os = "macos") {
        app.manage(MenuItems::default());
        return Ok(());
    }
    let (menu, items) = build(app.handle())?;
    app.set_menu(menu)?;
    app.manage(items);
    app.on_menu_event(|app, event| {
        let id = event.id().as_ref();
        if app.state::<MenuItems>().0.contains_key(id) {
            // Emitting only fails while the webview is gone.
            let _ = MenuCommand { id: id.to_owned() }.emit(app);
        }
    });
    Ok(())
}

/// Apply the webview's enabled state and shortcuts to the menu items.
/// Returns the ids whose accelerator the platform rejected.
#[tauri::command]
#[specta::specta]
pub(crate) fn menu_sync(items: State<'_, MenuItems>, states: Vec<MenuItemState>) -> Vec<String> {
    let mut rejected = Vec::new();
    for state in states {
        let Some(item) = items.0.get(&state.id) else {
            continue;
        };
        let enabled = item.set_enabled(state.enabled);
        let accelerator = item.set_accelerator(state.accelerator.as_deref());
        if enabled.is_err() || accelerator.is_err() {
            rejected.push(state.id);
        }
    }
    rejected
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every command id in the menu bar.
    fn command_ids(entries: &[Entry], ids: &mut Vec<&'static str>) {
        for entry in entries {
            match entry {
                Entry::Command(id, _) => ids.push(id),
                Entry::Submenu(_, nested) => command_ids(nested, ids),
                Entry::Separator | Entry::Native(_) => {}
            }
        }
    }

    #[test]
    fn command_ids_are_unique_and_cover_the_primary_actions() {
        let mut ids = Vec::new();
        for (_, entries) in MENUS {
            command_ids(entries, &mut ids);
        }
        let mut unique = ids.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), ids.len(), "duplicate menu ids");
        for id in [
            "settings",
            "new-tab",
            "open-file",
            "save",
            "save-as",
            "close-tab",
            "run-current",
            "run-all",
            "cancel",
            "beautify",
            "uglify",
            "insert-snippet",
            "connect",
            "disconnect",
            "database",
            "refresh",
            "processes",
            "open-logs",
            "commit-changes",
            "discard-changes",
            "insert-row",
            "delete-row",
            "copy-selection",
            "focus-filter",
            "toggle-columns",
            "export-results",
            "import-data",
            "help-search",
            "show-er-diagram",
        ] {
            assert!(ids.contains(&id), "missing {id}");
        }
    }
}
