// SPDX-License-Identifier: Apache-2.0
//! Incremental serde visitors replace rows with empty arrays in the envelope.
//! Body chunks and Arrow batches cross bounded channels; discarded rows use
//! `IgnoredAny`. A response byte ceiling bounds individual JSON strings too.
mod reader;
mod rows;
mod tree;
use crate::{ResultLimits, error, protocol::Reply};
use openrows_driver_api::DriverError;
use serde::de::DeserializeSeed;
use std::io::Read;
use tokio::sync::mpsc;

pub(crate) enum Event {
    Begin(arrow_schema::SchemaRef),
    Batch(arrow_array::RecordBatch, std::time::Duration),
    Truncated(usize),
}

pub(crate) use reader::ChunkReader;

pub(crate) fn parse(
    reader: impl Read,
    events: mpsc::Sender<Event>,
    limits: ResultLimits,
) -> Result<Reply, DriverError> {
    let mut decoder =
        serde_json::Deserializer::from_reader(reader.take(limits.response_bytes as u64 + 1));
    let value = tree::Tree {
        events,
        limits,
        scope: tree::Scope::Pipeline,
    }
    .deserialize(&mut decoder)
    .map_err(|_| error::protocol())?;
    decoder.end().map_err(|_| error::protocol())?;
    serde_json::from_value(value).map_err(|_| error::protocol())
}

pub(super) fn send(events: &mpsc::Sender<Event>, event: Event) -> Result<(), DriverError> {
    events
        .blocking_send(event)
        .map_err(|_| error::lost("libSQL response consumer stopped"))
}

#[cfg(test)]
mod tests;
