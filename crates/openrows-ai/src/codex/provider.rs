// SPDX-License-Identifier: Apache-2.0
//! Codex as an assistant provider behind the shared bounded agent loop.
//!
//! The CLI keeps one turn open while it waits for an application tool. Each
//! loop round either starts that turn or answers its pending tool calls with
//! the loop's gate-filtered results, so budgets, validation, traces, consent
//! and audit stay exactly those of every other provider.
use super::{
    Bridge,
    session::{Session, Step},
    status::supported,
};
use crate::{
    AiControl, AiError, ProviderConfig, RequestOptions,
    agent::{AgentProvider, Turn, TurnRequest},
};
use openrows_net::BoxFuture;
use tokio::sync::{Mutex, mpsc};

/// One conversation's Codex app-server process, started on the first round.
pub struct CodexProvider {
    bridge: Bridge,
    config: ProviderConfig,
    control: AiControl,
    tools: bool,
    deadline: std::time::Duration,
    session: Mutex<Option<Session>>,
}
impl CodexProvider {
    /// Build without spawning; the process starts with the first round.
    #[must_use]
    pub fn new(bridge: Bridge, config: ProviderConfig, control: AiControl) -> Self {
        Self {
            bridge,
            config,
            control,
            tools: true,
            deadline: crate::agent::CLI_TURN_DEFAULT,
            session: Mutex::new(None),
        }
    }
    /// Offer no application tools, for single-shot proposals.
    #[must_use]
    pub fn with_tool_calling(mut self, supported: bool) -> Self {
        self.tools = supported;
        self
    }
    /// Use the user's CLI turn deadline, clamped to the documented range.
    #[must_use]
    pub fn with_deadline(mut self, deadline: std::time::Duration) -> Self {
        self.deadline = deadline.clamp(
            crate::agent::AgentLimits::default().timeout,
            crate::agent::CLI_TURN_MAX,
        );
        self
    }
    async fn advance(
        &self,
        request: TurnRequest<'_>,
        sink: mpsc::Sender<String>,
    ) -> Result<Turn, AiError> {
        let mut slot = self.session.lock().await;
        let step = match slot.as_mut() {
            Some(session) => match session.reply(request.messages).await {
                Ok(()) => session.drive(&sink).await,
                Err(error) => Err(error),
            },
            None if request.messages.is_empty() => {
                let launch = self.bridge.launch()?;
                supported(&launch).await.map_err(|_| AiError::Unsupported)?;
                let session = slot.insert(Session::start(&launch, &request).await?);
                session.drive(&sink).await
            }
            None => Err(AiError::InvalidResponse),
        };
        match step {
            Ok(Step::Calls(calls)) => Ok(Turn {
                calls,
                ..Turn::default()
            }),
            Ok(Step::Finished) => {
                if let Some(session) = slot.take() {
                    session.close().await;
                }
                Ok(Turn::default())
            }
            Err(error) => {
                // Dropping the session kills the CLI's whole process group.
                slot.take();
                Err(error)
            }
        }
    }
}
impl AgentProvider for CodexProvider {
    fn tool_calling(&self) -> bool {
        self.tools
    }
    fn deadline(&self) -> std::time::Duration {
        self.deadline
    }
    fn turn<'a>(
        &'a self,
        request: TurnRequest<'a>,
        sink: mpsc::Sender<String>,
        options: &'a RequestOptions,
    ) -> BoxFuture<'a, Result<Turn, AiError>> {
        let context = request.context;
        Box::pin(async move {
            self.control
                .run(
                    options,
                    context.outbound(&self.config, self.advance(request, sink)),
                )
                .await
        })
    }
}
