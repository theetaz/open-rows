// SPDX-License-Identifier: Apache-2.0
//! One HTTP/1.1 request per socket; dropping a request closes its connection.
use crate::{
    ResultLimits, error,
    parse::{self, Event},
    protocol::{Pipeline, Reply},
    transport::Transport,
};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{
    Request as HttpRequest,
    body::Incoming,
    client::conn::http1,
    header::{AUTHORIZATION, CONTENT_TYPE, HOST},
};
use hyper_util::rt::TokioIo;
use openrows_driver_api::{
    ConnectTrace, DriverError, ExecSummary, NoticeLevel, ResultSink, SinkFlow,
};
use openrows_net::SecretString;
use tokio::{sync::mpsc, task::JoinHandle};

type Sender = http1::SendRequest<Full<Bytes>>;

pub(crate) struct Socket {
    sender: Sender,
    task: JoinHandle<()>,
}

impl Drop for Socket {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Socket {
    async fn open(transport: &Transport, trace: &mut ConnectTrace) -> Result<Self, DriverError> {
        let stream = transport.open(trace).await?;
        let (sender, connection) = http1::handshake(TokioIo::new(stream))
            .await
            .map_err(|_| error::lost("libSQL HTTP handshake failed"))?;
        let task = tokio::spawn(async move {
            let _ = connection.await;
        });
        Ok(Self { sender, task })
    }
}

pub(crate) struct Http {
    pub(crate) transport: Transport,
    pub(crate) token: Option<SecretString>,
    initial: Option<Socket>,
}

impl Http {
    pub(crate) async fn new(
        request: &openrows_driver_api::ConnectRequest,
        trace: &mut ConnectTrace,
    ) -> Result<Self, DriverError> {
        let transport = Transport::new(request)?;
        let initial = Some(Socket::open(&transport, trace).await?);
        Ok(Self {
            transport,
            token: request.password.clone(),
            initial,
        })
    }

    pub(crate) async fn request(
        &mut self,
        pipeline: &Pipeline<'_>,
        limits: ResultLimits,
        sink: &mut dyn ResultSink,
    ) -> Result<(Reply, ExecSummary), DriverError> {
        let mut socket = match self.initial.take() {
            Some(socket) => socket,
            None => Socket::open(&self.transport, &mut ConnectTrace::default()).await?,
        };
        let request = self.build(pipeline)?;
        let response = socket
            .sender
            .send_request(request)
            .await
            .map_err(|_| error::lost("libSQL HTTP request failed"))?;
        let status = response.status();
        if !status.is_success() {
            return Err(if status.as_u16() == 401 || status.as_u16() == 403 {
                DriverError::Auth("libSQL rejected the auth token".into())
            } else {
                error::lost(&format!(
                    "libSQL returned HTTP {status}; stream invalidated"
                ))
            });
        }
        decode(response.into_body(), limits, sink).await
    }

    fn build(&self, pipeline: &Pipeline<'_>) -> Result<HttpRequest<Full<Bytes>>, DriverError> {
        let url = self
            .transport
            .url
            .join("v3/pipeline")
            .map_err(|_| error::protocol())?;
        let body = serde_json::to_vec(pipeline).map_err(|_| error::protocol())?;
        let authority = &url[url::Position::BeforeHost..url::Position::AfterPort];
        let mut request = HttpRequest::builder()
            .method("POST")
            .uri(url.path())
            .header(HOST, authority)
            .header(CONTENT_TYPE, "application/json")
            .body(Full::new(Bytes::from(body)))
            .map_err(|_| error::protocol())?;
        if let Some(token) = &self.token {
            let mut header =
                hyper::header::HeaderValue::from_str(&format!("Bearer {}", token.expose()))
                    .map_err(|_| DriverError::Auth("invalid libSQL auth token header".into()))?;
            header.set_sensitive(true);
            request.headers_mut().insert(AUTHORIZATION, header);
        }
        Ok(request)
    }
}

async fn decode(
    body: Incoming,
    limits: ResultLimits,
    sink: &mut dyn ResultSink,
) -> Result<(Reply, ExecSummary), DriverError> {
    let (chunks, received) = mpsc::channel(8);
    let (events, mut output) = mpsc::channel(2);
    let parser = tokio::task::spawn_blocking(move || {
        parse::parse(
            std::io::BufReader::new(parse::ChunkReader::new(received)),
            events,
            limits,
        )
    });
    let read = async {
        pump(body, chunks, limits.response_bytes).await?;
        parser.await.map_err(|_| error::protocol())?
    };
    let forward = async {
        let mut summary = ExecSummary::default();
        let mut stopped = false;
        while let Some(event) = output.recv().await {
            deliver(event, sink, &mut summary, &mut stopped);
        }
        Ok(summary)
    };
    let (reply, summary) = tokio::try_join!(read, forward)?;
    Ok((reply, summary))
}

async fn pump(
    mut body: Incoming,
    chunks: mpsc::Sender<Bytes>,
    cap: usize,
) -> Result<(), DriverError> {
    let mut bytes = 0_usize;
    while let Some(frame) = body.frame().await {
        let frame = frame.map_err(|_| error::lost("libSQL HTTP body interrupted"))?;
        let Ok(data) = frame.into_data() else {
            continue;
        };
        bytes = bytes.saturating_add(data.len());
        if bytes > cap {
            return Err(DriverError::Unsupported(format!(
                "libSQL response exceeds {cap} bytes; add LIMIT"
            )));
        }
        for offset in (0..data.len()).step_by(16 * 1024) {
            let chunk = data.slice(offset..(offset + 16 * 1024).min(data.len()));
            chunks.send(chunk).await.map_err(|_| error::protocol())?;
        }
    }
    Ok(())
}

fn deliver(event: Event, sink: &mut dyn ResultSink, summary: &mut ExecSummary, stopped: &mut bool) {
    match event {
        Event::Begin(schema) if !*stopped => sink.begin_result(schema),
        Event::Batch(batch, latency) if !*stopped => {
            summary.first_batch_latency.get_or_insert(latency);
            summary.rows += batch.num_rows() as u64;
            summary.batches += 1;
            *stopped = sink.push_batch(batch) == SinkFlow::Stop;
        }
        Event::Truncated(cap) => sink.notice(
            NoticeLevel::Warning,
            &format!("result truncated at {cap} rows; add LIMIT"),
        ),
        Event::Begin(_) | Event::Batch(..) => {}
    }
}

#[cfg(test)]
mod tests;
