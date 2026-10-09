// SPDX-License-Identifier: Apache-2.0
//! Typed export failures.
use arrow_schema::DataType;

/// Why an export did not produce its file.
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    /// Creating, writing, syncing or renaming the file failed.
    #[error("the export file could not be written: {0}")]
    Io(#[from] std::io::Error),
    /// The destination has no file name (for example a bare directory).
    #[error("the export destination is not a file path")]
    InvalidPath,
    /// A column has an Arrow type the encoders do not handle.
    #[error("column {column:?} has type {data_type}, which cannot be exported yet")]
    UnsupportedType {
        /// Column name.
        column: String,
        /// Its Arrow type.
        data_type: DataType,
    },
    /// A table or column name cannot be written as an SQL identifier.
    #[error("{0:?} cannot be used as an SQL name because it contains a NUL character")]
    InvalidName(String),
    /// SQL `INSERT` needs at least one column.
    #[error("the result has no columns to insert")]
    NoColumns,
    /// The statement returned no result set.
    #[error("the statement returned no rows to export")]
    NoResult,
    /// A batch arrived before the schema or does not match it.
    #[error("a batch does not match the exported columns")]
    SchemaMismatch,
    /// The stream ended without finishing (cancelled or failed upstream).
    #[error("the export stopped before it finished")]
    Aborted,
}
