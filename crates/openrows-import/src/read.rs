// SPDX-License-Identifier: Apache-2.0
//! Streaming parsers producing raw records: delimited fields (through the
//! `csv` crate: RFC 4180 quoting, embedded newlines, any ASCII separator)
//! and JSON objects with their keys in file order (an array of objects, or
//! one object per line). Values keep their exact text: numbers and nested
//! JSON are never re-encoded.
use std::io::{BufRead, Read};
use std::ops::ControlFlow;

use serde::de::{self, Deserializer as _, MapAccess, SeqAccess, Visitor};
use serde_json::value::RawValue;

use crate::ImportError;

/// One record as parsed, before it is matched to columns.
#[derive(Debug)]
pub(crate) enum Raw {
    /// Delimited fields in file order.
    Fields(Vec<String>),
    /// JSON object members in file order; `None` is JSON `null`.
    Object(Vec<(String, Option<String>)>),
    /// A record that cannot be read, with the reason.
    Bad(String),
}

/// A raw record and the line (or JSON array record number) it starts on.
#[derive(Debug)]
pub(crate) struct RawRecord {
    pub(crate) line: u64,
    pub(crate) raw: Raw,
}

/// Receives each record; `Break` stops reading.
pub(crate) type Visit<'a> = dyn FnMut(RawRecord) -> ControlFlow<()> + 'a;

/// Read delimited records from UTF-8 text.
///
/// # Errors
/// [`ImportError::Io`] when reading fails.
pub(crate) fn delimited(
    reader: impl Read,
    (delimiter, quote): (u8, Option<u8>),
    visit: &mut Visit<'_>,
) -> Result<(), ImportError> {
    let mut parser = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .quote(quote.unwrap_or(b'"'))
        .quoting(quote.is_some())
        .has_headers(false)
        .flexible(true)
        .from_reader(reader);
    let mut record = csv::StringRecord::new();
    loop {
        let line = parser.position().line();
        let raw = match parser.read_record(&mut record) {
            Ok(false) => return Ok(()),
            Ok(true) => Raw::Fields(record.iter().map(str::to_owned).collect()),
            Err(error) if error.is_io_error() => {
                return Err(match error.into_kind() {
                    csv::ErrorKind::Io(error) => ImportError::Io(error),
                    _ => ImportError::Invalid("the file could not be read".to_owned()),
                });
            }
            Err(error) => Raw::Bad(error.to_string()),
        };
        let line = record.position().map_or(line, csv::Position::line);
        if visit(RawRecord { line, raw }).is_break() {
            return Ok(());
        }
    }
}

/// Read one JSON object per line; blank lines are skipped.
///
/// # Errors
/// [`ImportError::Io`] when reading fails.
pub(crate) fn json_lines(
    mut reader: impl BufRead,
    visit: &mut Visit<'_>,
) -> Result<(), ImportError> {
    let mut text = String::new();
    let mut line = 0;
    loop {
        text.clear();
        if reader.read_line(&mut text)? == 0 {
            return Ok(());
        }
        line += 1;
        if text.trim().is_empty() {
            continue;
        }
        let raw = match serde_json::from_str::<Object>(&text) {
            Ok(object) => Raw::Object(object.0),
            Err(error) => Raw::Bad(cause(&error)),
        };
        if visit(RawRecord { line, raw }).is_break() {
            return Ok(());
        }
    }
}

/// Read the elements of one top-level JSON array, streaming.
///
/// # Errors
/// [`ImportError::Json`] when the file is not an array or not valid JSON
/// (after that nothing can be read), [`ImportError::Io`].
pub(crate) fn json_array(reader: impl Read, visit: &mut Visit<'_>) -> Result<(), ImportError> {
    let mut deserializer = serde_json::Deserializer::from_reader(reader);
    let mut stopped = false;
    let elements = Elements {
        visit,
        stopped: &mut stopped,
    };
    match deserializer.deserialize_seq(elements) {
        Ok(()) => deserializer.end().map_err(ImportError::from),
        // Stopping early leaves the array open; that is not an error.
        Err(_) if stopped => Ok(()),
        Err(error) => Err(error.into()),
    }
}

/// `serde_json`'s message without its position suffix.
fn cause(error: &serde_json::Error) -> String {
    let message = error.to_string();
    message
        .split(" at line ")
        .next()
        .unwrap_or_default()
        .to_owned()
}

/// A JSON value as cell text: strings unescaped, `null` as `None`, numbers,
/// booleans and nested documents exactly as written.
fn cell(raw: &RawValue) -> Result<Option<String>, serde_json::Error> {
    let text = raw.get().trim();
    match text.as_bytes().first() {
        Some(b'"') => serde_json::from_str::<String>(text).map(Some),
        Some(b'n') => Ok(None),
        _ => Ok(Some(text.to_owned())),
    }
}

/// One JSON object with its members in file order.
struct Object(Vec<(String, Option<String>)>);

impl<'de> de::Deserialize<'de> for Object {
    fn deserialize<D: de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(ObjectVisitor)
    }
}

struct ObjectVisitor;

impl<'de> Visitor<'de> for ObjectVisitor {
    type Value = Object;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a JSON object")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Object, A::Error> {
        let mut members = Vec::new();
        while let Some((key, value)) = map.next_entry::<String, Box<RawValue>>()? {
            members.push((key, cell(&value).map_err(de::Error::custom)?));
        }
        Ok(Object(members))
    }
}

/// Visits array elements one by one, handing each to `visit`.
struct Elements<'v, 'a> {
    visit: &'v mut Visit<'a>,
    stopped: &'v mut bool,
}

impl<'de> Visitor<'de> for Elements<'_, '_> {
    type Value = ();

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a JSON array of objects")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        let mut line = 0;
        while let Some(element) = seq.next_element::<Box<RawValue>>()? {
            line += 1;
            let raw = match serde_json::from_str::<Object>(element.get()) {
                Ok(object) => Raw::Object(object.0),
                Err(error) => Raw::Bad(cause(&error)),
            };
            if (self.visit)(RawRecord { line, raw }).is_break() {
                *self.stopped = true;
                return Err(de::Error::custom("stopped"));
            }
        }
        Ok(())
    }
}
