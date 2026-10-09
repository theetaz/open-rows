// SPDX-License-Identifier: Apache-2.0
//! A file as rows of named columns: [`inspect`] detects the parse options,
//! names the columns, infers their types over a sample and keeps the first
//! rows for the preview; [`stream`] reads every row with fixed columns.
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::ops::ControlFlow;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use crate::decode::{Counting, Decoded, encoding, sniff_encoding};
use crate::infer::ColumnStats;
use crate::options::{FileFormat, ParseOptions};
use crate::read::{self, Raw, RawRecord, Visit};
use crate::{ImportError, RowError, detect};

/// Bytes read from the start of a file to detect its options.
const HEAD: usize = 64 * 1024;

/// One row of the file with a cell per column; `None` is NULL.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Record {
    /// 1-based line the row starts on (record number in a JSON array).
    pub line: u64,
    /// Cells in column order.
    pub cells: Vec<Option<String>>,
}

/// A row read from the file, or why it cannot be.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Item {
    /// A well-formed row.
    Record(Record),
    /// A malformed row (wrong field count, invalid JSON, not an object).
    Bad(RowError),
}

/// A column of the file and what the sample showed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceColumn {
    /// Header name, JSON key or `column_N`.
    pub name: String,
    /// Inferred category, NULLs and longest value.
    pub stats: ColumnStats,
}

/// How many rows [`inspect`] keeps and samples.
#[derive(Clone, Copy, Debug)]
pub struct SampleLimits {
    /// Rows kept for the preview.
    pub preview: usize,
    /// Rows read for type inference (at least `preview`).
    pub sample: usize,
}

/// What [`inspect`] found.
#[derive(Clone, Debug)]
pub struct Inspection {
    /// The options used: the given ones, or the detected ones.
    pub options: ParseOptions,
    /// Columns in file order.
    pub columns: Vec<SourceColumn>,
    /// The first rows, for the preview.
    pub rows: Vec<Record>,
    /// Malformed rows within the sample.
    pub errors: Vec<RowError>,
    /// Rows read for inference.
    pub sampled: usize,
}

/// Detect options (unless given), name the columns, infer their types over
/// the first `limits.sample` rows and keep the first `limits.preview`.
///
/// # Errors
/// I/O failures, an unknown encoding, an invalid JSON array or no columns.
pub fn inspect(
    path: &Path,
    options: Option<ParseOptions>,
    limits: SampleLimits,
) -> Result<Inspection, ImportError> {
    let options = match options {
        Some(options) => options,
        None => detect_options(path)?,
    };
    let mut raws = Vec::new();
    let sample = limits.sample.max(limits.preview).saturating_add(1);
    read_raw(path, &options, None, &mut |raw| {
        raws.push(raw);
        if raws.len() >= sample {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    })?;
    let names = column_names(&options, &raws);
    if names.is_empty() {
        return Err(ImportError::NoColumns);
    }
    let shape = Shape::new(&options, &names);
    let mut inspection = Inspection {
        columns: names
            .into_iter()
            .map(|name| SourceColumn {
                name,
                stats: ColumnStats::default(),
            })
            .collect(),
        options,
        rows: Vec::new(),
        errors: Vec::new(),
        sampled: 0,
    };
    for raw in raws.into_iter().skip(usize::from(shape.skip_first)) {
        inspection.observe(shape.item(raw), limits);
    }
    Ok(inspection)
}

impl Inspection {
    fn observe(&mut self, item: Item, limits: SampleLimits) {
        if self.sampled >= limits.sample.max(limits.preview) {
            return;
        }
        self.sampled += 1;
        match item {
            Item::Bad(error) => self.errors.push(error),
            Item::Record(record) => {
                for (column, cell) in self.columns.iter_mut().zip(&record.cells) {
                    column.stats.observe(cell.as_deref());
                }
                if self.rows.len() < limits.preview {
                    self.rows.push(record);
                }
            }
        }
    }
}

/// Read every row of the file as `columns` (the names [`inspect`] found),
/// counting raw bytes into `read` for progress.
///
/// # Errors
/// I/O failures, an unknown encoding or an invalid JSON array.
pub fn stream(
    path: &Path,
    (options, columns): (&ParseOptions, &[String]),
    read: Arc<AtomicU64>,
    visit: &mut dyn FnMut(Item) -> ControlFlow<()>,
) -> Result<(), ImportError> {
    let shape = Shape::new(options, columns);
    let mut first = shape.skip_first;
    read_raw(path, options, Some(read), &mut |raw| {
        if std::mem::take(&mut first) {
            return ControlFlow::Continue(());
        }
        visit(shape.item(raw))
    })
}

fn detect_options(path: &Path) -> Result<ParseOptions, ImportError> {
    let mut head = Vec::with_capacity(HEAD);
    File::open(path)?.take(HEAD as u64).read_to_end(&mut head)?;
    let encoding = sniff_encoding(&head);
    let mut text = String::new();
    Decoded::new(head.as_slice(), encoding).read_to_string(&mut text)?;
    let extension = path.extension().and_then(|extension| extension.to_str());
    let defaults = ParseOptions::default();
    let format = detect::format(extension, &text);
    let delimiter = detect::delimiter(&text, defaults.quote, extension);
    let mut first = Vec::new();
    if format == FileFormat::Delimited {
        read::delimited(text.as_bytes(), (delimiter, defaults.quote), &mut |raw| {
            if let Raw::Fields(fields) = raw.raw {
                first = fields;
            }
            ControlFlow::Break(())
        })?;
    }
    Ok(ParseOptions {
        format,
        delimiter,
        header: format != FileFormat::Delimited || detect::header(&first),
        encoding: encoding.name().to_ascii_lowercase(),
        ..defaults
    })
}

fn read_raw(
    path: &Path,
    options: &ParseOptions,
    bytes_read: Option<Arc<AtomicU64>>,
    visit: &mut Visit<'_>,
) -> Result<(), ImportError> {
    let encoding = encoding(&options.encoding)?;
    let file = Counting::new(File::open(path)?, bytes_read.unwrap_or_default());
    let text = BufReader::with_capacity(HEAD, Decoded::new(file, encoding));
    match options.format {
        FileFormat::Delimited => read::delimited(text, (options.delimiter, options.quote), visit),
        FileFormat::JsonArray => read::json_array(text, visit),
        FileFormat::JsonLines => read::json_lines(text, visit),
    }
}

/// Column names: the header (blank ones become `column_N`, repeats get a
/// suffix), `column_1…` without one, or JSON keys in first-seen order.
fn column_names(options: &ParseOptions, raws: &[RawRecord]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for raw in raws {
        match &raw.raw {
            Raw::Fields(fields) if options.header => {
                return unique(fields.iter().map(|field| field.trim().to_owned()));
            }
            Raw::Fields(fields) => {
                return unique((0..fields.len()).map(|_| String::new()));
            }
            Raw::Object(members) => {
                let new: Vec<String> = members
                    .iter()
                    .map(|(key, _)| key)
                    .filter(|key| !names.contains(key))
                    .cloned()
                    .collect();
                names.extend(new);
            }
            Raw::Bad(_) => {}
        }
    }
    names
}

fn unique(names: impl Iterator<Item = String>) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for (index, name) in names.enumerate() {
        let base = if name.is_empty() {
            format!("column_{}", index + 1)
        } else {
            name
        };
        let mut candidate = base.clone();
        let mut suffix = 2;
        while seen.contains(&candidate) {
            candidate = format!("{base}_{suffix}");
            suffix += 1;
        }
        seen.push(candidate);
    }
    seen
}

/// Turns raw records into rows of the fixed columns.
struct Shape {
    width: usize,
    index: HashMap<String, usize>,
    skip_first: bool,
    empty_as_null: bool,
}

impl Shape {
    fn new(options: &ParseOptions, columns: &[String]) -> Self {
        Self {
            width: columns.len(),
            index: columns
                .iter()
                .enumerate()
                .map(|(index, name)| (name.clone(), index))
                .collect(),
            skip_first: options.format == FileFormat::Delimited && options.header,
            empty_as_null: options.empty_as_null,
        }
    }

    fn cell(&self, value: Option<String>) -> Option<String> {
        value.filter(|text| !(self.empty_as_null && text.is_empty()))
    }

    fn item(&self, raw: RawRecord) -> Item {
        let line = raw.line;
        match raw.raw {
            Raw::Fields(fields) if fields.len() == self.width => Item::Record(Record {
                line,
                cells: fields
                    .into_iter()
                    .map(|field| self.cell(Some(field)))
                    .collect(),
            }),
            Raw::Fields(fields) => Item::Bad(RowError::row(
                line,
                format!("expected {} fields but found {}", self.width, fields.len()),
            )),
            Raw::Object(members) => {
                let mut cells = vec![None; self.width];
                for (slot, value) in members.into_iter().filter_map(|(key, value)| {
                    let slot = self.index.get(&key).copied()?;
                    Some((slot, value))
                }) {
                    cells[slot] = self.cell(value);
                }
                Item::Record(Record { line, cells })
            }
            Raw::Bad(message) => Item::Bad(RowError::row(line, message)),
        }
    }
}
