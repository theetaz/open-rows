// SPDX-License-Identifier: Apache-2.0
//! Conservative accounting includes repeated tool evidence, not just output text.
use super::{
    AgentError, AgentEvent, AgentLimits, AgentProvider, AgentRequest, Message, ToolDefinition,
    Turn, TurnRequest,
};
use crate::{AiError, sharing::GatedPayload};
use tokio::sync::mpsc;
pub(super) struct Budget {
    limits: AgentLimits,
    steps: u32,
    used: u32,
}
impl Budget {
    pub(super) fn new(limits: AgentLimits) -> Self {
        Self {
            limits,
            steps: 0,
            used: 0,
        }
    }
    pub(super) fn step(&mut self) -> Result<(), AgentError> {
        self.steps += 1;
        if self.steps > self.limits.steps {
            return Err(AgentError::Steps);
        }
        Ok(())
    }
    pub(super) fn used(&self) -> u32 {
        self.used
    }
    pub(super) fn steps(&self) -> u32 {
        self.steps
    }
    fn charge(&mut self, bytes: usize) -> Result<(), AgentError> {
        self.used = self
            .used
            .saturating_add(u32::try_from(bytes).unwrap_or(u32::MAX));
        if self.used >= self.limits.tokens {
            return Err(AgentError::Tokens);
        }
        Ok(())
    }
    pub(super) fn input(
        &mut self,
        context: &GatedPayload,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<(), AgentError> {
        self.charge(context.text().len() + super::calls::INSTRUCTIONS.len())?;
        self.charge(
            serde_json::to_vec(tools)
                .map_err(|_| AgentError::Malformed)?
                .len(),
        )?;
        for message in messages {
            let size = match message {
                Message::Assistant {
                    text,
                    calls,
                    continuation,
                } => {
                    text.len()
                        + serde_json::to_vec(continuation)
                            .map_err(|_| AgentError::Malformed)?
                            .len()
                        + serde_json::to_vec(calls)
                            .map_err(|_| AgentError::Malformed)?
                            .len()
                }
                Message::Tool { data, .. } => data.len(),
            };
            self.charge(size + 128)?;
        }
        Ok(())
    }
    pub(super) async fn round(
        &mut self,
        provider: &dyn AgentProvider,
        request: &AgentRequest,
        (messages, tools): (&[Message], &[ToolDefinition]),
        sink: &mpsc::Sender<AgentEvent>,
    ) -> Result<(Turn, String), AgentError> {
        let (send, mut receive) = mpsc::channel(16);
        let input = TurnRequest {
            model: &request.model,
            context: &request.context,
            messages,
            tools,
            max_output: (self.limits.tokens - self.used).min(4096),
        };
        let future = provider.turn(input, send, &request.options);
        tokio::pin!(future);
        let mut text = String::new();
        loop {
            tokio::select! {
                biased;
                delta = receive.recv() => if let Some(delta) = delta {
                    self.charge(delta.len())?;
                    text.push_str(&delta);
                    sink.send(AgentEvent::Text { delta }).await.map_err(|_| AiError::Cancelled)?;
                } else { break; },
                result = &mut future => {
                    let turn = result?;
                    while let Ok(delta) = receive.try_recv() {
                        self.charge(delta.len())?; text.push_str(&delta);
                        sink.send(AgentEvent::Text { delta }).await.map_err(|_| AiError::Cancelled)?;
                    }
                    self.charge(serde_json::to_vec(&(&turn.calls, &turn.continuation)).map_err(|_| AgentError::Malformed)?.len())?;
                    return Ok((turn, text));
                }
            }
        }
        let turn = future.await?;
        self.charge(
            serde_json::to_vec(&(&turn.calls, &turn.continuation))
                .map_err(|_| AgentError::Malformed)?
                .len(),
        )?;
        Ok((turn, text))
    }
}
pub(super) fn validate(request: &AgentRequest) -> Result<(), AgentError> {
    let hard = AgentLimits::default();
    if request.limits.steps == 0
        || request.limits.steps > hard.steps
        || request.limits.tokens == 0
        || request.limits.tokens > hard.tokens
        || request.limits.timeout.is_zero()
        || request.limits.timeout > hard.timeout
        || request.model.is_empty()
        || request.model.len() > 256
    {
        return Err(AgentError::Malformed);
    }
    Ok(())
}
