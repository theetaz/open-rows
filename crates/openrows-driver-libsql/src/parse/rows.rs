// SPDX-License-Identifier: Apache-2.0
//! Row-by-row Hrana values into the shared `SQLite` Arrow codec.
use super::{Event, send};
use crate::protocol::Value;
use openrows_driver_api::{BatchPolicy, DriverError};
use openrows_driver_sqlite::decode::{BatchBuffer, ColumnSpec};
use serde::de::{self, DeserializeSeed, SeqAccess, Visitor};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;

pub(super) struct Rows<'a> {
    pub(super) columns: Option<Arc<[ColumnSpec]>>,
    pub(super) events: &'a mpsc::Sender<Event>,
    pub(super) cap: usize,
}

pub(super) struct Deferred {
    pub(super) rows: Vec<Vec<Value>>,
    pub(super) truncated: bool,
    pub(super) streamed: bool,
}

impl<'de> DeserializeSeed<'de> for Rows<'_> {
    type Value = Deferred;
    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<Deferred, D::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for Rows<'_> {
    type Value = Deferred;
    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Hrana rows")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Deferred, A::Error> {
        let streamed = self.columns.is_some();
        let mut publisher = self
            .columns
            .map(|columns| Publisher::new(columns, self.events));
        let mut deferred = Deferred {
            rows: Vec::new(),
            truncated: false,
            streamed,
        };
        let mut count = 0;
        while count < self.cap {
            let Some(row) = sequence.next_element::<Vec<Value>>()? else {
                break;
            };
            count += 1;
            if let Some(publisher) = &mut publisher {
                publisher.push(row).map_err(de::Error::custom)?;
            } else {
                deferred.rows.push(row);
            }
        }
        while sequence.next_element::<de::IgnoredAny>()?.is_some() {
            deferred.truncated = true;
        }
        if let Some(publisher) = &mut publisher {
            publisher.finish().map_err(de::Error::custom)?;
        }
        Ok(deferred)
    }
}

pub(super) struct Publisher<'a> {
    buffer: BatchBuffer,
    width: usize,
    events: &'a mpsc::Sender<Event>,
    schema: Option<arrow_schema::SchemaRef>,
    started: Instant,
}

impl<'a> Publisher<'a> {
    pub(super) fn new(columns: Arc<[ColumnSpec]>, events: &'a mpsc::Sender<Event>) -> Self {
        Self {
            width: columns.len(),
            buffer: BatchBuffer::new(columns),
            events,
            schema: None,
            started: Instant::now(),
        }
    }

    pub(super) fn push(&mut self, row: Vec<Value>) -> Result<(), DriverError> {
        if row.len() != self.width {
            return Err(crate::error::protocol());
        }
        let cells = row
            .into_iter()
            .enumerate()
            .map(|(column, value)| value.into_cell(column))
            .collect::<Result<Vec<_>, _>>()?;
        self.buffer.push(cells);
        if self.buffer.rows() >= BatchPolicy::default().row_limit(self.width, true)
            || self.buffer.bytes() >= 4 * 1024 * 1024
        {
            self.flush()?;
        }
        Ok(())
    }

    pub(super) fn finish(&mut self) -> Result<(), DriverError> {
        if self.width > 0 && (self.buffer.rows() > 0 || self.schema.is_none()) {
            self.flush()?;
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<(), DriverError> {
        let batch = self.buffer.finish()?;
        let schema = batch.schema();
        if self.schema.as_ref() != Some(&schema) {
            self.schema = Some(Arc::clone(&schema));
            send(self.events, Event::Begin(schema))?;
        }
        if batch.num_rows() > 0 {
            send(self.events, Event::Batch(batch, self.started.elapsed()))?;
        }
        Ok(())
    }
}
