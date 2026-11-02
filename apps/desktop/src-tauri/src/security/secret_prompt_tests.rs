// SPDX-License-Identifier: Apache-2.0
use super::*;
use std::sync::{Arc, Mutex};

/// Answers like the dialog would: a typed value, or cancel.
pub(crate) struct FakePrompt {
    pub(crate) answer: Option<&'static str>,
    pub(crate) asked: Mutex<Vec<String>>,
}
impl FakePrompt {
    pub(crate) fn new(answer: Option<&'static str>) -> Self {
        Self {
            answer,
            asked: Mutex::default(),
        }
    }
}
impl SecretPrompt for FakePrompt {
    fn ask<'a>(
        &'a self,
        text: &'a PromptText,
    ) -> BoxFuture<'a, Result<Option<SecretString>, IpcError>> {
        if let Ok(mut asked) = self.asked.lock() {
            asked.push(text.title.clone());
        }
        let answer = self.answer.map(|value| SecretString::new(value.to_owned()));
        Box::pin(async move { Ok(answer) })
    }
}

fn text() -> PromptText {
    PromptText {
        title: "API key for Fixture".into(),
        message: "Stored in the keychain".into(),
    }
}

type Stored = Arc<Mutex<Vec<String>>>;
fn recorder(
    stored: &Stored,
) -> impl FnOnce(SecretString) -> BoxFuture<'static, Result<(), IpcError>> {
    let stored = Arc::clone(stored);
    move |secret| {
        Box::pin(async move {
            stored
                .lock()
                .map_err(|_| IpcError::invalid("poisoned"))?
                .push(secret.expose().to_owned());
            Ok(())
        })
    }
}

#[tokio::test]
async fn a_typed_secret_is_stored_and_only_the_outcome_is_returned() -> Result<(), IpcError> {
    let prompt = FakePrompt::new(Some("test-only-key"));
    let stored = Stored::default();
    let entry = enter(&prompt, &text(), recorder(&stored)).await?;
    assert_eq!(entry, SecretEntry::Saved);
    assert_eq!(
        *stored.lock().map_err(|_| IpcError::invalid("poisoned"))?,
        ["test-only-key"]
    );
    let wire = serde_json::to_string(&entry).map_err(|_| IpcError::invalid("json"))?;
    assert_eq!(wire, "\"saved\"");
    Ok(())
}

#[tokio::test]
async fn cancelling_stores_nothing() -> Result<(), IpcError> {
    let prompt = FakePrompt::new(None);
    let stored = Stored::default();
    assert_eq!(
        enter(&prompt, &text(), recorder(&stored)).await?,
        SecretEntry::Cancelled
    );
    assert!(
        stored
            .lock()
            .map_err(|_| IpcError::invalid("poisoned"))?
            .is_empty()
    );
    Ok(())
}

#[tokio::test]
async fn empty_oversized_or_control_values_are_refused_unstored() {
    let long: &'static str = Box::leak("k".repeat(MAX_SECRET + 1).into_boxed_str());
    for value in ["", "line\nbreak", "tab\tkey", long] {
        let prompt = FakePrompt::new(Some(value));
        let stored = Stored::default();
        let result = enter(&prompt, &text(), recorder(&stored)).await;
        assert!(result.is_err(), "{} accepted", value.len());
        assert!(stored.lock().is_ok_and(|stored| stored.is_empty()));
    }
}

#[tokio::test]
async fn a_failed_keychain_write_is_reported_as_an_error() {
    let prompt = FakePrompt::new(Some("test-only-key"));
    let result = enter(&prompt, &text(), |_| {
        Box::pin(async { Err(IpcError::new(IpcErrorKind::Secret, "keychain failed")) })
    })
    .await;
    assert!(result.is_err_and(|error| error.kind == IpcErrorKind::Secret));
}
