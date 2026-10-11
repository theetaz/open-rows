// SPDX-License-Identifier: Apache-2.0
#![allow(
    dead_code,
    reason = "shared fixture helpers compiled independently by each integration binary"
)]
pub mod sharing;
use openrows_ai::{AiControl, HttpProvider, NetworkOptions, ProviderConfig, ProviderKind};
use openrows_secrets::SecretString;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::mpsc,
    task::JoinHandle,
};
pub type TestResult = Result<(), Box<dyn std::error::Error>>;
pub struct Mock {
    pub url: String,
    pub requests: mpsc::Receiver<String>,
    task: JoinHandle<()>,
}
impl Drop for Mock {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Mock {
    pub async fn new(replies: Vec<String>) -> Result<Self, Box<dyn std::error::Error>> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let url = format!("http://{}/", listener.local_addr()?);
        let (send, requests) = mpsc::channel(32);
        let task = tokio::spawn(async move {
            let _ = serve(listener, send, replies).await;
        });
        Ok(Self {
            url,
            requests,
            task,
        })
    }
    pub fn context(
        &self,
        kind: ProviderKind,
    ) -> Result<openrows_ai::sharing::GatedPayload, Box<dyn std::error::Error>> {
        use openrows_ai::sharing::{
            ContextBundle, GateRequest, SharingLevel, SharingPolicy, SharingStore,
        };
        let store = SharingStore::memory(1000)?;
        store.save(
            "connection",
            SharingPolicy {
                level: SharingLevel::SchemaOnly,
                providers: vec!["fixture".into()],
                ..SharingPolicy::default()
            },
            false,
        )?;
        Ok(store.gate(GateRequest {
            connection: "connection",
            production: false,
            provider: &self.config(kind),
            control: &enabled(),
            dialect: openrows_sql::Dialect::Postgres,
            bundle: ContextBundle {
                query: Some("SELECT 'hello'".into()),
                ..ContextBundle::default()
            },
        })?)
    }
    pub fn provider(
        &self,
        kind: ProviderKind,
        control: AiControl,
    ) -> Result<HttpProvider, openrows_ai::AiError> {
        HttpProvider::new(
            &self.config(kind),
            Some(SecretString::from("test-only")),
            control,
            NetworkOptions::default(),
        )
    }
    pub fn config(&self, kind: ProviderKind) -> ProviderConfig {
        ProviderConfig {
            id: "fixture".into(),
            name: "Fixture".into(),
            kind,
            base_url: self.url.clone(),
            allow_insecure_remote: false,
            secret_ref: "ai/fixture/api-key".into(),
        }
    }
}
fn complete(bytes: &[u8]) -> bool {
    let Some(end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") else {
        return false;
    };
    let headers = String::from_utf8_lossy(&bytes[..end]).to_ascii_lowercase();
    let length = headers
        .lines()
        .find_map(|line| {
            line.strip_prefix("content-length: ")
                .and_then(|value| value.parse::<usize>().ok())
        })
        .unwrap_or(0);
    bytes.len() >= end + 4 + length
}
pub fn response(status: u16, body: &str) -> String {
    format!(
        "HTTP/1.1 {status} fixture\r\nContent-Length: {}\r\nRetry-After: 7\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}
pub fn models(kind: ProviderKind) -> String {
    match kind {
        ProviderKind::Ollama => response(200, r#"{"models":[{"name":"model"}]}"#),
        ProviderKind::Gemini => response(
            200,
            r#"{"models":[{"name":"models/model","inputTokenLimit":4096,"outputTokenLimit":1024,"supportedGenerationMethods":["generateContent"]}]}"#,
        ),
        _ => response(
            200,
            r#"{"data":[{"id":"model","context_length":4096,"supported_parameters":["tools","response_format"],"architecture":{"input_modalities":["text","image"]}}]}"#,
        ),
    }
}
pub fn stream(kind: ProviderKind) -> String {
    let body = match kind {
        ProviderKind::Anthropic => {
            "data: {\"type\":\"content_block_delta\",\"delta\":{\"text\":\"Hello\"}}\r\n\r\ndata: {\"type\":\"message_stop\"}\r\n\r\n"
        }
        ProviderKind::OpenAi => {
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"Hello\"}\n\ndata: {\"type\":\"response.completed\"}\n\n"
        }
        ProviderKind::Gemini => {
            "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"Hello\"}]},\"finishReason\":\"STOP\"}]}\n\n"
        }
        ProviderKind::Ollama => {
            "{\"message\":{\"content\":\"Hello\"},\"done\":false}\n{\"done\":true}\n"
        }
        _ => "data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\ndata: [DONE]\n\n",
    };
    // Fragment every byte: exercises UTF-8/JSON delimiters across HTTP chunks.
    let mut chunks = String::new();
    for byte in body.bytes() {
        chunks.push_str("1\r\n");
        chunks.push(char::from(byte));
        chunks.push_str("\r\n");
    }
    format!("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n{chunks}0\r\n\r\n")
}
pub fn enabled() -> AiControl {
    let control = AiControl::default();
    control.set_enabled(true);
    control
}

async fn serve(
    listener: TcpListener,
    send: mpsc::Sender<String>,
    replies: Vec<String>,
) -> std::io::Result<()> {
    for reply in replies {
        let (mut stream, _) = listener.accept().await?;
        let bytes = read_request(&mut stream).await?;
        send.send(String::from_utf8_lossy(&bytes).into_owned())
            .await
            .map_err(|_| std::io::Error::other("fixture receiver closed"))?;
        if reply == "hang" {
            std::future::pending::<()>().await;
        }
        stream.write_all(reply.as_bytes()).await?;
    }
    Ok(())
}
async fn read_request(stream: &mut tokio::net::TcpStream) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    while !complete(&bytes) {
        let mut buffer = [0; 8192];
        let count = stream.read(&mut buffer).await?;
        if count == 0 || bytes.len() > 2 * 1024 * 1024 {
            return Err(std::io::Error::other("incomplete fixture request"));
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
    Ok(bytes)
}
