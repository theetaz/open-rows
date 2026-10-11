// SPDX-License-Identifier: Apache-2.0
//! Credential expiry and disconnected-client generation tests.
use super::*;
#[test]
fn expired_saved_tokens_are_refused_and_disconnect_cancels_admitted_work() -> Result<(), McpError> {
    let store = TokenStore::memory()?;
    let created = store.create(TokenScope {
        provider: "provider".into(),
        connections: vec!["connection".into()],
        expires: now()? + 60,
    })?;
    let grant = store.authenticate(&created.value)?;
    store.disconnect(&created.info.id);
    assert!(grant.cancel.is_cancelled());
    let newer = store.authenticate(&created.value)?;
    drop(grant);
    assert_eq!(store.clients()[0].active, 1);
    assert!(!newer.cancel.is_cancelled());
    let mut scope = created.info.scope;
    scope.expires = 1;
    store
        .database
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .execute(
            "UPDATE tokens SET scope=?1",
            [serde_json::to_string(&scope).map_err(|_| McpError::Store)?],
        )
        .map_err(|_| McpError::Store)?;
    assert!(matches!(
        store.authenticate(&created.value),
        Err(McpError::Auth)
    ));
    Ok(())
}
