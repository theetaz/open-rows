// SPDX-License-Identifier: Apache-2.0
//! Bound statements and conditional batches (Hrana 3 §Shared structures).
use base64::Engine as _;
use serde::{Deserialize, Serialize};

/// A native `SQLite` storage value, bound separately from SQL text.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Value {
    /// SQL NULL.
    Null,
    /// Signed integer encoded as decimal text to preserve all 64 bits.
    Integer {
        /// Decimal integer.
        value: String,
    },
    /// Double-precision real.
    Float {
        /// Real value.
        value: f64,
    },
    /// UTF-8 text.
    Text {
        /// Text value.
        value: String,
    },
    /// Binary bytes encoded with standard base64.
    Blob {
        /// Encoded bytes.
        base64: String,
    },
}

impl Value {
    /// Decode the native storage value using `SQLite`'s shared Arrow representation.
    /// # Errors
    /// Invalid 64-bit integer or base64 bytes with a value-free column diagnostic.
    pub fn into_cell(
        self,
        column: usize,
    ) -> Result<openrows_driver_sqlite::decode::Cell, openrows_driver_api::DriverError> {
        use openrows_driver_sqlite::decode::Cell;
        let failed = || openrows_driver_api::DriverError::Decode {
            column,
            reason: "invalid Hrana storage value".into(),
        };
        Ok(match self {
            Self::Null => Cell::Null,
            Self::Integer { value } => Cell::Integer(value.parse().map_err(|_| failed())?),
            Self::Float { value } => Cell::Real(value),
            Self::Text { value } => Cell::Text(value),
            Self::Blob { base64 } => Cell::Blob(
                base64::engine::general_purpose::STANDARD
                    .decode(&base64)
                    .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(&base64))
                    .map_err(|_| failed())?,
            ),
        })
    }

    pub(crate) fn text(value: Option<&str>) -> Self {
        value.map_or(Self::Null, |value| Self::Text {
            value: value.into(),
        })
    }
}

/// A native named binding, including its `SQLite` sigil when supplied.
#[derive(Clone, Debug, Serialize)]
pub struct NamedArg {
    /// Parameter name, e.g. `:name`.
    pub name: String,
    /// Bound storage value.
    pub value: Value,
}

#[derive(Serialize)]
pub(crate) struct Pipeline<'a> {
    pub(crate) baton: Option<&'a str>,
    pub(crate) requests: &'a [Request],
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum Request {
    Execute { stmt: Statement },
    Batch { batch: Batch },
    Sequence { sql: String },
    Close,
    GetAutocommit,
}

#[derive(Serialize)]
pub(crate) struct Statement {
    pub(crate) sql: String,
    pub(crate) args: Vec<Value>,
    pub(crate) named_args: Vec<NamedArg>,
    pub(crate) want_rows: bool,
}

impl Statement {
    pub(crate) fn text(sql: &str, params: &[Option<String>]) -> Self {
        Self {
            sql: sql.into(),
            args: params
                .iter()
                .map(|value| Value::text(value.as_deref()))
                .collect(),
            named_args: Vec::new(),
            want_rows: true,
        }
    }
}

#[derive(Serialize)]
pub(crate) struct Batch {
    pub(crate) steps: Vec<Step>,
}

#[derive(Serialize)]
pub(crate) struct Step {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) condition: Option<Condition>,
    pub(crate) stmt: Statement,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum Condition {
    Ok { step: usize },
    Not { cond: Box<Self> },
}

impl Batch {
    pub(crate) fn atomic(statements: &[openrows_sql::select::BoundSql]) -> Self {
        let mut steps = vec![Step {
            condition: None,
            stmt: Statement::text("BEGIN IMMEDIATE", &[]),
        }];
        for statement in statements {
            let previous = steps.len() - 1;
            steps.push(Step {
                condition: Some(Condition::Ok { step: previous }),
                stmt: Statement::text(&statement.sql, &statement.params),
            });
        }
        let last = steps.len() - 1;
        steps.push(Step {
            condition: Some(Condition::Ok { step: last }),
            stmt: Statement::text("COMMIT", &[]),
        });
        steps.push(Step {
            condition: Some(Condition::Not {
                cond: Box::new(Condition::Ok { step: last + 1 }),
            }),
            stmt: Statement::text("ROLLBACK", &[]),
        });
        Self { steps }
    }
}
