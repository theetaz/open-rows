// SPDX-License-Identifier: Apache-2.0
//! SSH prompts answered in native-looking sheets: the first-use host-key
//! decision and keyboard-interactive (MFA/OTP) challenges (docs/12 §4.2).
//!
//! The core emits `ssh-prompt-changed` with a prompt and waits; the webview
//! answers with `ssh_prompt_answer`. Answers travel webview → core only. A
//! prompt that is abandoned (connect failed, timed out) is closed with a
//! second event without a prompt.
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use openrows_net::{
    BoxFuture, Challenge, FirstUseDecision, HostKeyPolicy, KeyboardInteractivePrompter,
    PromptCancelled, SecretString, UnknownHost,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;
use tokio::sync::oneshot;

use super::Connections;
use super::error::{IpcError, IpcErrorKind};

/// How long an unanswered prompt waits before it counts as cancelled.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(300);

/// One line of a keyboard-interactive challenge.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChallengePromptDto {
    text: String,
    /// The answer may be shown while typing (otherwise it is masked).
    echo: bool,
}

/// What the user is asked.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum SshPromptDto {
    /// An SSH server key seen for the first time.
    #[serde(rename_all = "camelCase")]
    HostKey {
        host: String,
        port: u16,
        algorithm: String,
        /// `SHA256:…`
        fingerprint: String,
        /// Algorithms already trusted for this host (a reason for caution).
        other_algorithms: Vec<String>,
    },
    /// Keyboard-interactive questions (password, verification code, …).
    #[serde(rename_all = "camelCase")]
    Challenge {
        host: String,
        user: String,
        name: String,
        instructions: String,
        prompts: Vec<ChallengePromptDto>,
    },
}

/// Opens (`prompt` set) or closes (`prompt` null) a prompt sheet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SshPromptChanged {
    id: String,
    prompt: Option<SshPromptDto>,
}

impl tauri_specta::Event for SshPromptChanged {
    const NAME: &'static str = "ssh-prompt-changed";
}

/// The user's answer. `Debug` redacts challenge answers (passwords, codes).
#[derive(Clone, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum SshPromptAnswer {
    /// Trust the host key and remember it in the app's `known_hosts`.
    Trust,
    /// Connect this time without saving the key; the connection is flagged
    /// as insecure while the tunnel lives.
    TrustOnce,
    /// Answers to a challenge, one per prompt, in order.
    Answers { values: Vec<String> },
    /// Dismissed: reject the key or cancel authentication.
    Cancel,
}

impl std::fmt::Debug for SshPromptAnswer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Trust => formatter.write_str("Trust"),
            Self::TrustOnce => formatter.write_str("TrustOnce"),
            Self::Answers { values } => write!(formatter, "Answers([{} redacted])", values.len()),
            Self::Cancel => formatter.write_str("Cancel"),
        }
    }
}

/// Delivers prompt events to the webview.
pub(crate) trait PromptSink: Send + Sync {
    /// Show or close a prompt; false when nobody can see it.
    fn send(&self, event: SshPromptChanged) -> bool;
}

/// Emits through the Tauri event system.
pub(crate) struct TauriSink(pub(crate) tauri::AppHandle);

impl PromptSink for TauriSink {
    fn send(&self, event: SshPromptChanged) -> bool {
        tauri_specta::Event::emit(&event, &self.0).is_ok()
    }
}

/// Pending prompts and their answer channels.
pub(crate) struct PromptBroker {
    sink: Box<dyn PromptSink>,
    pending: Mutex<HashMap<String, oneshot::Sender<SshPromptAnswer>>>,
    next: AtomicU64,
}

impl PromptBroker {
    pub(crate) fn new(sink: Box<dyn PromptSink>) -> Self {
        Self {
            sink,
            pending: Mutex::new(HashMap::new()),
            next: AtomicU64::new(1),
        }
    }

    /// Show `prompt` and wait for the answer; `Cancel` when it times out,
    /// cannot be shown, or the waiting connect is dropped.
    pub(crate) async fn ask(&self, prompt: SshPromptDto) -> SshPromptAnswer {
        let id = format!("ssh-{}", self.next.fetch_add(1, Ordering::Relaxed));
        let (sender, receiver) = oneshot::channel();
        self.lock().insert(id.clone(), sender);
        let _close = Closer {
            broker: self,
            id: id.clone(),
        };
        if !self.sink.send(SshPromptChanged {
            id,
            prompt: Some(prompt),
        }) {
            return SshPromptAnswer::Cancel;
        }
        match tokio::time::timeout(ANSWER_TIMEOUT, receiver).await {
            Ok(Ok(answer)) => answer,
            Ok(Err(_)) | Err(_) => SshPromptAnswer::Cancel,
        }
    }

    /// Deliver the webview's answer to prompt `id`.
    ///
    /// # Errors
    /// `NotFound` when the prompt is no longer open.
    pub(crate) fn answer(&self, id: &str, answer: SshPromptAnswer) -> Result<(), IpcError> {
        let sender = self.lock().remove(id).ok_or_else(|| {
            IpcError::new(IpcErrorKind::NotFound, "This prompt is no longer open")
        })?;
        // The connect may have given up in the meantime; nothing waits then.
        let _ = sender.send(answer);
        Ok(())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, oneshot::Sender<SshPromptAnswer>>> {
        self.pending.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Closes the webview's sheet when a prompt ends for any reason.
struct Closer<'a> {
    broker: &'a PromptBroker,
    id: String,
}

impl Drop for Closer<'_> {
    fn drop(&mut self) {
        self.broker.lock().remove(&self.id);
        self.broker.sink.send(SshPromptChanged {
            id: std::mem::take(&mut self.id),
            prompt: None,
        });
    }
}

/// Host-key first use through the prompt sheet: Trust remembers the key,
/// Connect once does not.
pub(crate) struct UiHostKeyPolicy(pub(crate) Arc<PromptBroker>);

impl HostKeyPolicy for UiHostKeyPolicy {
    fn on_unknown_host<'a>(&'a self, host: &'a UnknownHost) -> BoxFuture<'a, FirstUseDecision> {
        Box::pin(async move {
            let prompt = SshPromptDto::HostKey {
                host: host.host.clone(),
                port: host.port,
                algorithm: host.algorithm.clone(),
                fingerprint: host.fingerprint.clone(),
                other_algorithms: host.other_algorithms.clone(),
            };
            match self.0.ask(prompt).await {
                SshPromptAnswer::Trust => FirstUseDecision::AcceptAndRemember,
                SshPromptAnswer::TrustOnce => FirstUseDecision::AcceptOnce,
                SshPromptAnswer::Answers { .. } | SshPromptAnswer::Cancel => {
                    FirstUseDecision::Reject
                }
            }
        })
    }
}

/// Keyboard-interactive challenges through the prompt sheet.
pub(crate) struct UiPrompter(pub(crate) Arc<PromptBroker>);

impl KeyboardInteractivePrompter for UiPrompter {
    fn respond<'a>(
        &'a self,
        challenge: &'a Challenge,
    ) -> BoxFuture<'a, Result<Vec<SecretString>, PromptCancelled>> {
        Box::pin(async move {
            let prompt = SshPromptDto::Challenge {
                host: challenge.host.clone(),
                user: challenge.user.clone(),
                name: challenge.name.clone(),
                instructions: challenge.instructions.clone(),
                prompts: challenge
                    .prompts
                    .iter()
                    .map(|prompt| ChallengePromptDto {
                        text: prompt.text.clone(),
                        echo: prompt.echo,
                    })
                    .collect(),
            };
            match self.0.ask(prompt).await {
                SshPromptAnswer::Answers { values } if values.len() == challenge.prompts.len() => {
                    Ok(values.into_iter().map(SecretString::new).collect())
                }
                SshPromptAnswer::Answers { .. }
                | SshPromptAnswer::Trust
                | SshPromptAnswer::TrustOnce
                | SshPromptAnswer::Cancel => Err(PromptCancelled),
            }
        })
    }
}

/// Answer an open SSH prompt (host-key trust or challenge answers).
#[tauri::command]
#[specta::specta]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri deserialises command arguments into owned values"
)]
pub(crate) fn ssh_prompt_answer(
    state: State<'_, Connections>,
    id: String,
    answer: SshPromptAnswer,
) -> Result<(), IpcError> {
    state.prompts.answer(&id, answer)
}

#[cfg(test)]
mod tests;
