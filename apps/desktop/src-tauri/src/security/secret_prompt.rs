// SPDX-License-Identifier: Apache-2.0
//! Native secure text entry (docs/23 §10). A secret typed into the system
//! dialog goes straight to the OS keychain: it never crosses IPC, and the
//! webview learns only whether it was saved or cancelled.
use crate::connections::{IpcError, IpcErrorKind};
use openrows_driver_api::BoxFuture;
use openrows_secrets::SecretString;
use serde::Serialize;
use specta::Type;

/// Longest secret accepted from the dialog.
const MAX_SECRET: usize = 4096;

/// What the dialog shows; nothing here is secret.
pub(crate) struct PromptText {
    /// Bold first line, such as "API key for Work Anthropic".
    pub(crate) title: String,
    /// Explanation below the title.
    pub(crate) message: String,
}

/// Outcome reported to the webview; the secret itself is never returned.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum SecretEntry {
    /// The value was written to the OS keychain.
    Saved,
    /// The user dismissed the dialog; nothing changed.
    Cancelled,
}

/// The OS-owned entry boundary; tests substitute a fake.
pub(crate) trait SecretPrompt: Send + Sync {
    /// Ask for one secret; `Ok(None)` when the user cancels.
    fn ask<'a>(
        &'a self,
        text: &'a PromptText,
    ) -> BoxFuture<'a, Result<Option<SecretString>, IpcError>>;
}

/// Ask, validate and hand the value to `store`; cancelling stores nothing.
pub(crate) async fn enter(
    prompt: &dyn SecretPrompt,
    text: &PromptText,
    store: impl FnOnce(SecretString) -> BoxFuture<'static, Result<(), IpcError>>,
) -> Result<SecretEntry, IpcError> {
    let Some(secret) = prompt.ask(text).await? else {
        return Ok(SecretEntry::Cancelled);
    };
    validate(&secret)?;
    store(secret).await?;
    Ok(SecretEntry::Saved)
}

/// Non-empty, bounded and free of control characters.
fn validate(secret: &SecretString) -> Result<(), IpcError> {
    let value = secret.expose();
    if value.is_empty() || value.len() > MAX_SECRET || value.chars().any(char::is_control) {
        return Err(IpcError::invalid(
            "The entered value is empty, too long or contains control characters",
        ));
    }
    Ok(())
}

/// Production prompt: an `NSAlert` with an `NSSecureTextField` on the main thread.
pub(crate) struct NativePrompt {
    pub(crate) app: tauri::AppHandle,
}

impl SecretPrompt for NativePrompt {
    fn ask<'a>(
        &'a self,
        text: &'a PromptText,
    ) -> BoxFuture<'a, Result<Option<SecretString>, IpcError>> {
        let title = text.title.clone();
        let message = text.message.clone();
        Box::pin(async move {
            let (send, receive) = tokio::sync::oneshot::channel();
            self.app
                .run_on_main_thread(move || {
                    let _ = send.send(native::ask(&title, &message));
                })
                .map_err(|_| stopped())?;
            receive.await.map_err(|_| stopped())?
        })
    }
}

fn stopped() -> IpcError {
    IpcError::new(IpcErrorKind::Internal, "The secure entry dialog stopped")
}

#[cfg(target_os = "macos")]
mod native {
    use super::{IpcError, IpcErrorKind, SecretString};
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSAlert, NSAlertFirstButtonReturn, NSApplication, NSSecureTextField};
    use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

    /// Runs the modal dialog; must be called on the main thread.
    pub(super) fn ask(title: &str, message: &str) -> Result<Option<SecretString>, IpcError> {
        let mtm = MainThreadMarker::new().ok_or_else(|| {
            IpcError::new(IpcErrorKind::Internal, "Secure entry needs the main thread")
        })?;
        let alert = NSAlert::new(mtm);
        alert.setMessageText(&NSString::from_str(title));
        alert.setInformativeText(&NSString::from_str(message));
        alert.addButtonWithTitle(&NSString::from_str("Save"));
        alert.addButtonWithTitle(&NSString::from_str("Cancel"));
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(320.0, 24.0));
        let field = NSSecureTextField::initWithFrame(mtm.alloc(), frame);
        alert.setAccessoryView(Some(&field));
        alert.layout();
        alert.window().setInitialFirstResponder(Some(&field));
        NSApplication::sharedApplication(mtm).activate();
        let saved = alert.runModal() == NSAlertFirstButtonReturn;
        let value = field.stringValue().to_string();
        field.setStringValue(&NSString::from_str(""));
        Ok(saved.then(|| SecretString::new(value)))
    }
}

#[cfg(not(target_os = "macos"))]
mod native {
    use super::{IpcError, IpcErrorKind, SecretString};

    pub(super) fn ask(_title: &str, _message: &str) -> Result<Option<SecretString>, IpcError> {
        Err(IpcError::new(
            IpcErrorKind::Unsupported,
            "Secure entry is not available on this platform yet",
        ))
    }
}

#[cfg(test)]
#[path = "secret_prompt_tests.rs"]
pub(crate) mod tests;
