// SPDX-License-Identifier: Apache-2.0
//! Bounded JSON-RPC over the child's stdio: one JSON object per line, with the
//! `"jsonrpc":"2.0"` member omitted as the app-server protocol specifies.
use super::{
    args,
    process::{Isolated, Launch},
};
use crate::AiError;
use serde_json::{Value, json};
use std::collections::VecDeque;
use tokio::{
    io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader},
    process::{ChildStdin, ChildStdout},
};

/// Largest single message accepted from the CLI.
const LINE_BYTES: usize = 1 << 20;
/// Total output accepted from one CLI process.
const TOTAL_BYTES: usize = 32 << 20;
/// Server messages buffered while a client request waits for its response.
const QUEUE: usize = 256;

/// A server-initiated message.
#[derive(Debug)]
pub(crate) enum Inbound {
    Notification {
        method: String,
        params: Value,
    },
    Request {
        id: Value,
        method: String,
        params: Value,
    },
}

/// One initialized app-server connection; dropping it kills the process group.
pub(crate) struct Rpc {
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    queue: VecDeque<Inbound>,
    next: u64,
    read: usize,
    pub(crate) user_agent: String,
    child: Isolated,
}
impl Rpc {
    /// Spawn the isolated app-server and complete the `initialize` handshake.
    pub(crate) async fn connect(launch: &Launch, experimental: bool) -> Result<Self, AiError> {
        let (child, stdin, stdout) = Isolated::spawn(launch.command(&args::app_server())?)?;
        let mut rpc = Self {
            stdin,
            stdout: BufReader::new(stdout),
            queue: VecDeque::new(),
            next: 0,
            read: 0,
            user_agent: String::new(),
            child,
        };
        let capabilities = json!({
            "experimentalApi": experimental,
            "requestAttestation": false,
            "optOutNotificationMethods": args::QUIET,
        });
        let client =
            json!({"name": "openrows", "title": "OpenRows", "version": env!("CARGO_PKG_VERSION")});
        let result = rpc
            .request(
                "initialize",
                json!({"clientInfo": client, "capabilities": capabilities}),
            )
            .await?;
        rpc.user_agent = result["userAgent"]
            .as_str()
            .filter(|agent| agent.len() <= 512)
            .ok_or(AiError::InvalidResponse)?
            .to_owned();
        rpc.notify("initialized", json!({})).await?;
        Ok(rpc)
    }
    /// Send one request and wait for its response, queueing interleaved messages.
    pub(crate) async fn request(&mut self, method: &str, params: Value) -> Result<Value, AiError> {
        let id = self.next;
        self.next += 1;
        self.write(&json!({"id": id, "method": method, "params": params}))
            .await?;
        loop {
            let message = self.read().await?;
            if message.get("method").is_none() && message.get("id") == Some(&json!(id)) {
                return outcome(message);
            }
            if message.get("method").is_none() {
                continue;
            }
            if self.queue.len() >= QUEUE {
                return Err(AiError::InvalidResponse);
            }
            self.queue.push_back(inbound(message)?);
        }
    }
    /// The next server notification or request; stray responses are skipped.
    pub(crate) async fn next(&mut self) -> Result<Inbound, AiError> {
        if let Some(message) = self.queue.pop_front() {
            return Ok(message);
        }
        loop {
            let message = self.read().await?;
            if message.get("method").is_some() {
                return inbound(message);
            }
        }
    }
    pub(crate) async fn notify(&mut self, method: &str, params: Value) -> Result<(), AiError> {
        self.write(&json!({"method": method, "params": params}))
            .await
    }
    pub(crate) async fn respond(&mut self, id: Value, result: Value) -> Result<(), AiError> {
        self.write(&json!({"id": id, "result": result})).await
    }
    pub(crate) async fn reject(&mut self, id: Value, message: &str) -> Result<(), AiError> {
        self.write(&json!({"id": id, "error": {"code": -32601, "message": message}}))
            .await
    }
    /// Terminate and reap the process group.
    pub(crate) async fn close(self) {
        drop(self.stdin);
        self.child.stop().await;
    }
    async fn write(&mut self, message: &Value) -> Result<(), AiError> {
        let mut line = serde_json::to_vec(message).map_err(|_| AiError::InvalidConfig)?;
        line.push(b'\n');
        self.stdin
            .write_all(&line)
            .await
            .map_err(|_| AiError::Network)?;
        self.stdin.flush().await.map_err(|_| AiError::Network)
    }
    /// Read one non-empty line within the per-message and per-process bounds.
    async fn read(&mut self) -> Result<Value, AiError> {
        let mut line = Vec::new();
        loop {
            let buffer = self.stdout.fill_buf().await.map_err(|_| AiError::Network)?;
            // End of output means the CLI exited or crashed.
            if buffer.is_empty() {
                return Err(AiError::Network);
            }
            let end = buffer.iter().position(|byte| *byte == b'\n');
            let take = end.map_or(buffer.len(), |index| index + 1);
            line.extend_from_slice(&buffer[..take]);
            self.stdout.consume(take);
            self.read += take;
            if line.len() > LINE_BYTES || self.read > TOTAL_BYTES {
                return Err(AiError::InvalidResponse);
            }
            if end.is_some() && line.iter().all(u8::is_ascii_whitespace) {
                line.clear();
            } else if end.is_some() {
                return serde_json::from_slice(&line).map_err(|_| AiError::InvalidResponse);
            }
        }
    }
}
fn inbound(mut message: Value) -> Result<Inbound, AiError> {
    let method = message["method"]
        .as_str()
        .filter(|method| method.len() <= 128)
        .ok_or(AiError::InvalidResponse)?
        .to_owned();
    let params = message.get_mut("params").map_or(Value::Null, Value::take);
    Ok(match message.get_mut("id").map(Value::take) {
        Some(id) if id.is_string() || id.is_number() => Inbound::Request { id, method, params },
        Some(_) => return Err(AiError::InvalidResponse),
        None => Inbound::Notification { method, params },
    })
}
/// Map a response to its result; error text from the CLI is never surfaced.
fn outcome(mut message: Value) -> Result<Value, AiError> {
    let Some(error) = message.get("error") else {
        return message
            .get_mut("result")
            .map(Value::take)
            .ok_or(AiError::InvalidResponse);
    };
    let text = error["message"].as_str().unwrap_or_default();
    Err(match error["code"].as_i64() {
        Some(-32001) => AiError::RateLimit { retry_after: None },
        Some(-32601) => AiError::Unsupported,
        Some(-32600)
            if text.contains("unknown variant") || text.contains("requires experimentalApi") =>
        {
            AiError::Unsupported
        }
        _ => AiError::InvalidResponse,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn errors_map_to_fixed_kinds_without_cli_text() {
        let error = |code: i64, message: &str| json!({"id": 1, "error": {"code": code, "message": message}});
        assert_eq!(
            outcome(error(-32001, "Server overloaded")),
            Err(AiError::RateLimit { retry_after: None })
        );
        assert_eq!(
            outcome(error(-32600, "Invalid request: unknown variant `x`")),
            Err(AiError::Unsupported)
        );
        assert_eq!(
            outcome(error(
                -32600,
                "dynamicTools requires experimentalApi capability"
            )),
            Err(AiError::Unsupported)
        );
        assert_eq!(
            outcome(error(-32603, "secret detail")),
            Err(AiError::InvalidResponse)
        );
        assert_eq!(
            outcome(json!({"id": 1, "result": {"ok": true}})),
            Ok(json!({"ok": true}))
        );
        assert!(matches!(
            inbound(json!({"method": "turn/started"})),
            Ok(Inbound::Notification { .. })
        ));
        assert!(matches!(
            inbound(json!({"id": "a", "method": "item/tool/call"})),
            Ok(Inbound::Request { .. })
        ));
        assert!(inbound(json!({"id": {}, "method": "x"})).is_err());
    }
}
