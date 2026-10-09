// SPDX-License-Identifier: Apache-2.0
//! Column mapping (FR-IO-02): each target column takes a source column or a
//! constant; target columns not mapped keep their defaults. Values are
//! checked against the target column's category and normalised for the
//! engine, so a bad value is reported with its line before any SQL runs.
use openrows_driver_api::TextRow;
use openrows_sql::Dialect;

use crate::source::Record;
use crate::types::TypeCategory;
use crate::{ImportError, RowError};

/// Characters of a rejected value quoted in its error.
const QUOTED: usize = 40;

/// Where a target column's value comes from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ColumnSource {
    /// The source column at this index.
    Column(usize),
    /// The same value for every row; `None` is NULL.
    Constant(Option<String>),
}

/// One target column and its source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MappedColumn {
    /// Target column name.
    pub target: String,
    /// Where its values come from.
    pub source: ColumnSource,
    /// Values it accepts.
    pub category: TypeCategory,
}

/// Turns file rows into rows of the mapped target columns.
#[derive(Clone, Debug)]
pub struct Mapper {
    columns: Vec<MappedColumn>,
    dialect: Dialect,
}

impl Mapper {
    /// A mapper for files with `width` source columns.
    ///
    /// # Errors
    /// [`ImportError::Invalid`] when nothing is mapped, a target repeats or a
    /// source index is out of range.
    pub fn new(
        columns: Vec<MappedColumn>,
        dialect: Dialect,
        width: usize,
    ) -> Result<Self, ImportError> {
        if columns.is_empty() {
            return Err(ImportError::Invalid("Map at least one column".to_owned()));
        }
        for (index, column) in columns.iter().enumerate() {
            if column.target.trim().is_empty() {
                return Err(ImportError::Invalid("Every column needs a name".to_owned()));
            }
            if columns[..index]
                .iter()
                .any(|other| other.target == column.target)
            {
                return Err(ImportError::Invalid(format!(
                    "Column {:?} is mapped twice",
                    column.target
                )));
            }
            if matches!(column.source, ColumnSource::Column(source) if source >= width) {
                return Err(ImportError::Invalid(format!(
                    "Column {:?} maps a source column the file does not have",
                    column.target
                )));
            }
        }
        Ok(Self { columns, dialect })
    }

    /// Target column names, in row order.
    #[must_use]
    pub fn targets(&self) -> Vec<String> {
        self.columns
            .iter()
            .map(|column| column.target.clone())
            .collect()
    }

    /// The mapped row, or the first value its target column rejects.
    ///
    /// # Errors
    /// A [`RowError`] naming the line and target column.
    pub fn row(&self, record: &Record) -> Result<TextRow, RowError> {
        self.columns
            .iter()
            .map(|column| {
                let value = match &column.source {
                    ColumnSource::Column(index) => record.cells.get(*index).cloned().flatten(),
                    ColumnSource::Constant(value) => value.clone(),
                };
                value
                    .map(|text| self.checked(column, record.line, text))
                    .transpose()
            })
            .collect()
    }

    fn checked(&self, column: &MappedColumn, line: u64, text: String) -> Result<String, RowError> {
        let trimmed = text.trim();
        if column.category == TypeCategory::Text {
            return Ok(text);
        }
        if !column.category.accepts(trimmed) {
            let quoted: String = text.chars().take(QUOTED).collect();
            let ellipsis = if text.chars().count() > QUOTED {
                "…"
            } else {
                ""
            };
            return Err(RowError {
                line,
                column: Some(column.target.clone()),
                message: format!(
                    "{quoted:?}{ellipsis} is not a valid {}",
                    column.category.label()
                ),
            });
        }
        Ok(column
            .category
            .normalise(self.dialect, trimmed)
            .into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column(target: &str, source: ColumnSource, category: TypeCategory) -> MappedColumn {
        MappedColumn {
            target: target.to_owned(),
            source,
            category,
        }
    }

    #[test]
    fn rows_take_sources_constants_and_checked_values() -> Result<(), ImportError> {
        let mapper = Mapper::new(
            vec![
                column("id", ColumnSource::Column(1), TypeCategory::Integer),
                column("active", ColumnSource::Column(0), TypeCategory::Boolean),
                column(
                    "origin",
                    ColumnSource::Constant(Some("csv".into())),
                    TypeCategory::Text,
                ),
                column("note", ColumnSource::Constant(None), TypeCategory::Text),
            ],
            Dialect::MySql,
            2,
        )?;
        assert_eq!(mapper.targets(), ["id", "active", "origin", "note"]);
        let record = Record {
            line: 7,
            cells: vec![Some("yes".into()), Some(" 42 ".into())],
        };
        assert_eq!(
            mapper.row(&record),
            Ok(vec![
                Some("42".into()),
                Some("1".into()),
                Some("csv".into()),
                None
            ])
        );
        let bad = Record {
            line: 9,
            cells: vec![None, Some("4x".into())],
        };
        let error = mapper.row(&bad).err();
        assert_eq!(
            error.map(|error| error.to_string()),
            Some("line 9, id: \"4x\" is not a valid integer".to_owned())
        );
        Ok(())
    }

    #[test]
    fn invalid_mappings_are_refused() {
        let id = column("id", ColumnSource::Column(0), TypeCategory::Text);
        assert!(Mapper::new(vec![], Dialect::Postgres, 1).is_err());
        assert!(Mapper::new(vec![id.clone(), id.clone()], Dialect::Postgres, 1).is_err());
        assert!(Mapper::new(vec![id.clone()], Dialect::Postgres, 0).is_err());
        let blank = column(" ", ColumnSource::Column(0), TypeCategory::Text);
        assert!(Mapper::new(vec![blank], Dialect::Postgres, 1).is_err());
        assert!(Mapper::new(vec![id], Dialect::Postgres, 1).is_ok());
    }
}
