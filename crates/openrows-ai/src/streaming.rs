// SPDX-License-Identifier: Apache-2.0
//! Incremental SSE/NDJSON framing bounded independently from total output.
use crate::{AiError, HttpProvider, ProviderKind, TextRequest, http, wire};
use http_body_util::BodyExt;
use tokio::sync::mpsc;
const MAX_FRAME: usize = 256 * 1024;
const MAX_TOTAL: usize = 16 * 1024 * 1024;

pub(crate) async fn stream(
    provider: &HttpProvider,
    request: &TextRequest,
    sink: mpsc::Sender<String>,
) -> Result<(), AiError> {
    let (path, body) = wire::request(provider.kind, request)?;
    request.context.record().await?;
    exchange(provider, (path, body), sink, false).await?;
    Ok(())
}
pub(crate) async fn turn(
    provider: &HttpProvider,
    request: &crate::agent::TurnRequest<'_>,
    sink: mpsc::Sender<String>,
) -> Result<crate::agent::Turn, AiError> {
    let body = crate::agent_wire::request(provider.kind, request)?;
    request.context.record().await?;
    exchange(provider, body, sink, true).await
}
async fn exchange(
    provider: &HttpProvider,
    (path, body): (String, serde_json::Value),
    sink: mpsc::Sender<String>,
    tools: bool,
) -> Result<crate::agent::Turn, AiError> {
    let mut exchange = provider.http.request(&path, Some(body)).await?;
    http::check_status(&mut exchange).await?;
    let mut decoder = Decoder::new(provider, sink);
    decoder.calls = tools.then(crate::tool_stream::Calls::default);
    let mut total = 0usize;
    while let Some(frame) = exchange.response.body_mut().frame().await {
        let frame = frame.map_err(|_| AiError::Network)?;
        if let Ok(data) = frame.into_data() {
            total = total.saturating_add(data.len());
            if total > MAX_TOTAL {
                return Err(AiError::InvalidResponse);
            }
            for chunk in data.chunks(8192) {
                decoder.push(chunk).await?;
            }
            if decoder.done {
                return decoder.result();
            }
        }
    }
    decoder.finish().await?;
    decoder.result()
}
struct Decoder<'a> {
    provider: &'a HttpProvider,
    sink: mpsc::Sender<String>,
    pending: Vec<u8>,
    event: Vec<u8>,
    done: bool,
    calls: Option<crate::tool_stream::Calls>,
}
impl<'a> Decoder<'a> {
    fn new(provider: &'a HttpProvider, sink: mpsc::Sender<String>) -> Self {
        Self {
            provider,
            sink,
            pending: Vec::new(),
            event: Vec::new(),
            done: false,
            calls: None,
        }
    }
    async fn push(&mut self, bytes: &[u8]) -> Result<(), AiError> {
        self.pending.extend_from_slice(bytes);
        while let Some(end) = self.pending.iter().position(|byte| *byte == b'\n') {
            let mut line: Vec<u8> = self.pending.drain(..=end).collect();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            self.line(&line).await?;
            if self.done {
                return Ok(());
            }
        }
        if self.pending.len() > MAX_FRAME || self.event.len() > MAX_FRAME {
            return Err(AiError::InvalidResponse);
        }
        Ok(())
    }
    async fn line(&mut self, line: &[u8]) -> Result<(), AiError> {
        if line.len() > MAX_FRAME {
            return Err(AiError::InvalidResponse);
        }
        if self.provider.kind == ProviderKind::Ollama {
            if !line.is_empty() {
                self.emit(line).await?;
            }
        } else if line.is_empty() {
            if !self.event.is_empty() {
                let event = std::mem::take(&mut self.event);
                self.emit(&event).await?;
            }
        } else if let Some(data) = line.strip_prefix(b"data:") {
            if !self.event.is_empty() {
                self.event.push(b'\n');
            }
            self.event
                .extend_from_slice(data.strip_prefix(b" ").unwrap_or(data));
            if self.event.len() > MAX_FRAME {
                return Err(AiError::InvalidResponse);
            }
        }
        Ok(())
    }
    async fn emit(&mut self, bytes: &[u8]) -> Result<(), AiError> {
        if bytes == b"[DONE]"
            && matches!(
                self.provider.kind,
                ProviderKind::Compatible | ProviderKind::LocalCompatible | ProviderKind::OpenRouter
            )
        {
            self.done = true;
            return Ok(());
        }
        let value = self.provider.http.json(bytes)?;
        if let Some(calls) = &mut self.calls {
            calls.frame(self.provider.kind, &value)?;
        }
        let (deltas, done) = wire::delta(self.provider.kind, &value)?;
        for text in deltas {
            if !text.is_empty() {
                self.sink.send(text).await.map_err(|_| AiError::Cancelled)?;
            }
        }
        self.done = done;
        Ok(())
    }
    async fn finish(&mut self) -> Result<(), AiError> {
        if !self.pending.is_empty() {
            let line = std::mem::take(&mut self.pending);
            self.line(&line).await?;
        }
        if !self.event.is_empty() {
            let event = std::mem::take(&mut self.event);
            self.emit(&event).await?;
        }
        if self.done {
            Ok(())
        } else {
            Err(AiError::InvalidResponse)
        }
    }
    fn result(&mut self) -> Result<crate::agent::Turn, AiError> {
        self.calls.take().map_or_else(
            || Ok(crate::agent::Turn::default()),
            crate::tool_stream::Calls::finish,
        )
    }
}
