// SPDX-License-Identifier: Apache-2.0
//! Window labels and the saved list of open windows. Labels are slots
//! (`main`, `workspace-2` … `workspace-12`): a slot's workspace state and
//! frame are stored under its label, so reusing a label restores them.
use openrows_store::{LocalStore, StoreError, WorkspaceKey};

use crate::queries::history::now_ms;
use crate::workspace::PARTS;

/// The first window, created from `tauri.conf.json`.
pub(crate) const MAIN: &str = "main";
const PREFIX: &str = "workspace-";
/// Most workspace windows open at once.
pub(crate) const MAX_WINDOWS: usize = 12;
/// Store key of the open-window list (not a workspace label).
const LIST: WorkspaceKey<'static> = WorkspaceKey {
    window: "app",
    part: "windows",
};

/// Every slot label in order: `main`, then `workspace-2` and up.
pub(crate) fn labels() -> impl Iterator<Item = String> {
    std::iter::once(MAIN.to_owned()).chain((2..=MAX_WINDOWS).map(|n| format!("{PREFIX}{n}")))
}

/// Whether `label` names a workspace window slot.
pub(crate) fn is_workspace(label: &str) -> bool {
    label == MAIN
        || label
            .strip_prefix(PREFIX)
            .and_then(|n| n.parse::<usize>().ok())
            .is_some_and(|n| (2..=MAX_WINDOWS).contains(&n) && label == format!("{PREFIX}{n}"))
}

/// The first slot no open window uses.
pub(crate) fn free(open: &[String]) -> Option<String> {
    labels().find(|label| !open.contains(label))
}

/// The saved open-window labels: valid, unique, in slot order.
pub(crate) fn load(store: &LocalStore) -> Vec<String> {
    let saved: Vec<String> = store
        .workspace_state(LIST)
        .ok()
        .flatten()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    labels().filter(|label| saved.contains(label)).collect()
}

/// Remember which windows are open for the next launch.
pub(crate) fn save(store: &LocalStore, open: &[String]) -> Result<(), StoreError> {
    let text = serde_json::to_string(open).unwrap_or_else(|_| "[]".to_owned());
    store.save_workspace_state(LIST, &text, now_ms())
}

/// Forget a closed window's workspace for good.
pub(crate) fn forget(store: &LocalStore, label: &str) -> Result<(), StoreError> {
    for part in PARTS {
        store.clear_workspace_state(WorkspaceKey {
            window: label,
            part,
        })?;
    }
    Ok(())
}

/// Move `from`'s workspace into `main`, which takes its place.
pub(crate) fn adopt(store: &LocalStore, from: &str) -> Result<(), StoreError> {
    for part in PARTS {
        let source = WorkspaceKey { window: from, part };
        let target = WorkspaceKey { window: MAIN, part };
        match store.workspace_state(source)? {
            Some(state) => store.save_workspace_state(target, &state, now_ms())?,
            None => store.clear_workspace_state(target)?,
        }
    }
    forget(store, from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_are_bounded_slots() {
        assert_eq!(labels().count(), MAX_WINDOWS);
        assert!(
            is_workspace("main") && is_workspace("workspace-2") && is_workspace("workspace-12")
        );
        for label in [
            "workspace-1",
            "workspace-13",
            "workspace-02",
            "workspace-x",
            "app",
            "Main",
        ] {
            assert!(!is_workspace(label), "{label}");
        }
        let open = vec!["main".to_owned(), "workspace-3".to_owned()];
        assert_eq!(free(&open).as_deref(), Some("workspace-2"));
        assert_eq!(free(&labels().collect::<Vec<_>>()), None);
    }

    #[test]
    fn closed_windows_are_forgotten_and_main_adopts_a_survivor()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let store = LocalStore::open(&dir.path().join("openrows.sqlite"))?;
        let key = |window| WorkspaceKey {
            window,
            part: "queryTabs",
        };
        store.save_workspace_state(key("main"), "old", 1)?;
        store.save_workspace_state(key("workspace-3"), "kept", 1)?;
        save(&store, &["workspace-3".to_owned(), "bogus".to_owned()])?;
        assert_eq!(load(&store), ["workspace-3"]);
        adopt(&store, "workspace-3")?;
        assert_eq!(store.workspace_state(key("main"))?.as_deref(), Some("kept"));
        assert_eq!(store.workspace_state(key("workspace-3"))?, None);
        forget(&store, "main")?;
        assert_eq!(store.workspace_state(key("main"))?, None);
        Ok(())
    }
}
