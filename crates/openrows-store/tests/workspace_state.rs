// SPDX-License-Identifier: Apache-2.0
//! Session and workspace state (v6): per window and part, replaced on save.

use openrows_store::{LocalStore, WorkspaceKey};

const TABS: WorkspaceKey<'static> = WorkspaceKey {
    window: "main",
    part: "queryTabs",
};
const SESSION: WorkspaceKey<'static> = WorkspaceKey {
    window: "main",
    part: "session",
};

#[test]
fn workspace_state_is_saved_replaced_cleared_and_survives_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("openrows.sqlite");
    let store = LocalStore::open(&path).unwrap();
    assert_eq!(store.workspace_state(TABS).unwrap(), None);
    store
        .save_workspace_state(TABS, r#"{"tabs":[]}"#, 1)
        .unwrap();
    store
        .save_workspace_state(TABS, r#"{"tabs":[1]}"#, 2)
        .unwrap();
    store
        .save_workspace_state(SESSION, r#"{"active":null}"#, 2)
        .unwrap();
    drop(store);

    let store = LocalStore::open(&path).unwrap();
    assert_eq!(
        store.workspace_state(TABS).unwrap().as_deref(),
        Some(r#"{"tabs":[1]}"#)
    );
    store.clear_workspace_state(TABS).unwrap();
    store.clear_workspace_state(TABS).unwrap();
    assert_eq!(store.workspace_state(TABS).unwrap(), None);
    assert!(store.workspace_state(SESSION).unwrap().is_some());
    let other = WorkspaceKey {
        window: "second",
        ..SESSION
    };
    assert_eq!(store.workspace_state(other).unwrap(), None);
}
