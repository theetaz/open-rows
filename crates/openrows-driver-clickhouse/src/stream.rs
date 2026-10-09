// SPDX-License-Identifier: Apache-2.0
//! Native Arrow IPC decoder on a blocking worker fed by bounded HTTP chunks.
use crate::http::Reply;
use arrow_array::RecordBatch;
use arrow_ipc::reader::StreamReader;
use arrow_schema::{Schema, SchemaRef};
use bytes::Bytes;
use http_body_util::BodyExt;
use openrows_driver_api::{
    ConnectRequest, DriverError, ExecSummary, ResultSink, SinkFlow, metadata::SOURCE_TYPE,
};
use std::{
    io::{self, Read},
    sync::{Arc, Mutex, OnceLock},
    time::Instant,
};
use tokio::sync::{mpsc, oneshot};

enum Event {
    Schema(SchemaRef),
    Batch(RecordBatch),
    Error(DriverError),
}

struct Chunks {
    receiver: mpsc::Receiver<io::Result<Bytes>>,
    remaining: Bytes,
    tail: Arc<Mutex<Bytes>>,
}
impl Read for Chunks {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        while self.remaining.is_empty() {
            let Some(chunk) = self.receiver.blocking_recv() else {
                return Ok(0);
            };
            self.remaining = chunk?;
            remember(&self.tail, &self.remaining);
        }

        let size = output.len().min(self.remaining.len());
        output[..size].copy_from_slice(&self.remaining.split_to(size));
        Ok(size)
    }
}

fn schema(source: &Schema, types: &[(String, String)]) -> SchemaRef {
    let fields = source
        .fields()
        .iter()
        .enumerate()
        .map(|(index, field)| {
            let mut metadata = field.metadata().clone();
            if let Some((_, kind)) = types.get(index) {
                metadata.insert(SOURCE_TYPE.to_owned(), kind.clone());
            }
            field.as_ref().clone().with_metadata(metadata)
        })
        .collect::<Vec<_>>();
    Arc::new(Schema::new_with_metadata(fields, source.metadata().clone()))
}

fn decode(
    chunks: Chunks,
    sender: &mpsc::Sender<Event>,
    types: &[(String, String)],
) -> Result<(), DriverError> {
    let reader = StreamReader::try_new(chunks, None)?;
    let schema = schema(&reader.schema(), types);
    if sender
        .blocking_send(Event::Schema(Arc::clone(&schema)))
        .is_err()
    {
        return Ok(());
    }
    for batch in reader {
        let batch = batch?;
        let batch = RecordBatch::try_new(Arc::clone(&schema), batch.columns().to_vec())?;
        if sender.blocking_send(Event::Batch(batch)).is_err() {
            break;
        }
    }
    Ok(())
}

pub(crate) async fn ingest(
    reply: Reply,
    types: Vec<(String, String)>,
    sink: &mut dyn ResultSink,
    config: Arc<ConnectRequest>,
) -> Result<ExecSummary, DriverError> {
    let tail = Arc::new(Mutex::new(Bytes::new()));
    let first_data = Arc::new(OnceLock::new());
    let arrived = Arc::clone(&first_data);
    let (chunks, receiver) = mpsc::channel(2);
    let (sender, mut events) = mpsc::channel(2);
    let (ready, started) = oneshot::channel();
    let worker = tokio::task::spawn_blocking(move || {
        let input = Chunks {
            receiver,
            remaining: Bytes::new(),
            tail: Arc::clone(&tail),
        };
        // Start the blocking reader before polling the body, keeping worker
        // creation outside the measured first-data-to-first-batch interval.
        let _ = ready.send(());
        if let Err(error) = decode(input, &sender, &types) {
            let mapped = stream_error(&tail, &config).unwrap_or(error);
            let _ = sender.blocking_send(Event::Error(mapped));
        }
    });
    started
        .await
        .map_err(|error| DriverError::Protocol(error.to_string()))?;
    let guard = Pump(tokio::spawn(pump(reply, chunks, arrived)));
    let outcome = receive(&mut events, sink, &first_data).await;
    drop(events);
    drop(guard);
    worker
        .await
        .map_err(|error| DriverError::Protocol(error.to_string()))?;
    outcome
}

struct Pump(tokio::task::JoinHandle<()>);
impl Drop for Pump {
    fn drop(&mut self) {
        self.0.abort();
    }
}

async fn receive(
    events: &mut mpsc::Receiver<Event>,
    sink: &mut dyn ResultSink,
    first_data: &OnceLock<Instant>,
) -> Result<ExecSummary, DriverError> {
    let mut summary = ExecSummary::default();
    while let Some(event) = events.recv().await {
        match event {
            Event::Schema(schema) => sink.begin_result(schema),
            Event::Batch(batch) => {
                if push(&batch, sink, &mut summary, first_data) {
                    return Ok(summary);
                }
            }
            Event::Error(error) => return Err(error),
        }
    }
    Ok(summary)
}

fn remember(tail: &Mutex<Bytes>, chunk: &Bytes) {
    if let Ok(mut last) = tail.lock() {
        let keep = last.len().min(65_536_usize.saturating_sub(chunk.len()));
        let mut bytes = Vec::with_capacity(keep + chunk.len());
        bytes.extend_from_slice(&last[last.len() - keep..]);
        bytes.extend_from_slice(chunk);
        *last = Bytes::from(bytes);
    }
}

fn stream_error(tail: &Mutex<Bytes>, config: &ConnectRequest) -> Option<DriverError> {
    let bytes = tail.lock().ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let start = text.rfind("Code: ")?;
    let body = text.get(start..)?;
    if !body.contains("DB::Exception:") {
        return None;
    }
    body.strip_prefix("Code: ")?
        .split_once('.')?
        .0
        .parse::<u32>()
        .ok()?;
    Some(crate::error::server(None, body, config))
}

async fn pump(
    mut reply: Reply,
    chunks: mpsc::Sender<io::Result<Bytes>>,
    arrived: Arc<OnceLock<Instant>>,
) {
    while let Some(frame) = reply.response.body_mut().frame().await {
        let data = match frame {
            Ok(frame) => frame.into_data().ok(),
            Err(error) => {
                let _ = chunks.send(Err(io::Error::other(error))).await;
                break;
            }
        };
        let Some(data) = data.filter(|data| !data.is_empty()) else {
            continue;
        };
        let _ = arrived.set(Instant::now());
        for chunk in data.chunks(65_536) {
            if chunks
                .send(Ok(Bytes::copy_from_slice(chunk)))
                .await
                .is_err()
            {
                return;
            }
        }
    }
}

fn push(
    batch: &RecordBatch,
    sink: &mut dyn ResultSink,
    summary: &mut ExecSummary,
    first_data: &OnceLock<Instant>,
) -> bool {
    let first = summary.batches == 0;
    let start = first_data.get().copied().unwrap_or_else(Instant::now);
    let mut offset = 0;
    while offset < batch.num_rows() {
        let size = (batch.num_rows() - offset).min(if first && offset == 0 { 200 } else { 65_536 });
        summary.rows += size as u64;
        summary.batches += 1;
        if first && offset == 0 {
            summary.first_batch_latency = Some(start.elapsed());
        }
        if sink.push_batch(batch.slice(offset, size)) == SinkFlow::Stop {
            return true;
        }
        offset += size;
    }
    false
}
