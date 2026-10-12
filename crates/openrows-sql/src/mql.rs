// SPDX-License-Identifier: Apache-2.0
//! Bounded `MongoDB` shell syntax parsing without JavaScript execution.
pub(crate) mod browse;
mod constructors;
pub(crate) mod edit;
mod scan;
#[cfg(test)]
mod tests;
mod value;

use crate::{Classification, StatementKind};
use serde_json::Value;

/// Parsed collection operation; arguments retain Extended JSON values.
#[derive(Clone, Debug, PartialEq)]
pub struct Query {
    /// Collection name, including names requiring `getCollection`.
    pub collection: String,
    /// Supported shell operation.
    pub method: String,
    /// Parsed arguments.
    pub arguments: Vec<Value>,
    /// Optional find sort.
    pub sort: Option<Value>,
    /// Optional result cap.
    pub limit: Option<u32>,
    /// Optional find offset.
    pub skip: u64,
    /// A terminal shell `toArray()` asks for whole-document presentation.
    pub document_view: bool,
}

/// Invalid or unsupported MQL, with no user values in its message.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("Invalid MQL: {0}")]
pub struct ParseError(pub &'static str);

/// Parse one shell-like collection operation. JavaScript is never evaluated.
/// # Errors
/// Invalid syntax, unknown methods, unsupported chains or oversized input.
pub fn parse(source: &str) -> Result<Query, ParseError> {
    scan::validate_size(source)?;
    let source = source.trim().trim_end_matches(';').trim();
    let mut rest = source
        .strip_prefix("db.")
        .ok_or(ParseError("expected db.collection"))?;
    let collection = scan::collection(&mut rest)?;
    let (method, arguments) = scan::call(&mut rest)?;
    validate_arguments(&method, &arguments)?;
    let mut query = Query {
        collection,
        method,
        arguments,
        sort: None,
        limit: None,
        skip: 0,
        document_view: false,
    };
    while !rest.trim().is_empty() {
        rest = rest
            .trim()
            .strip_prefix('.')
            .ok_or(ParseError("unexpected trailing code"))?;
        let (method, arguments) = scan::call(&mut rest)?;
        query.chain(&method, &arguments)?;
    }
    Ok(query)
}

fn validate_arguments(method: &str, args: &[Value]) -> Result<(), ParseError> {
    let valid = match method {
        "find" => args.len() <= 2 && args.iter().all(Value::is_object),
        "aggregate" | "insertMany" => args.len() == 1 && args[0].is_array(),
        "countDocuments" => args.len() <= 1 && args.iter().all(Value::is_object),
        "createIndex" => (1..=2).contains(&args.len()) && args.iter().all(Value::is_object),
        "insertOne" | "deleteOne" | "deleteMany" => {
            args.len() == 1 && args.iter().all(Value::is_object)
        }
        "updateOne" | "updateMany" | "replaceOne" => {
            args.len() == 2 && args.iter().all(Value::is_object)
        }
        "dropIndex" => args.len() == 1 && args[0].is_string(),
        "getIndexes" => args.is_empty(),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(ParseError("unsupported method or arguments"))
    }
}

impl Query {
    fn chain(&mut self, method: &str, args: &[Value]) -> Result<(), ParseError> {
        if self.document_view {
            return Err(ParseError("toArray must end the cursor chain"));
        }
        if method == "toArray"
            && args.is_empty()
            && matches!(self.method.as_str(), "find" | "aggregate")
        {
            self.document_view = true;
            return Ok(());
        }
        if self.method != "find" || args.len() != 1 {
            return Err(ParseError("invalid cursor chain"));
        }
        match method {
            "sort" if args[0].is_object() && self.sort.is_none() => {
                self.sort = Some(args[0].clone());
            }
            "limit" if self.limit.is_none() => {
                self.limit = Some(
                    args[0]
                        .as_u64()
                        .and_then(|n| u32::try_from(n).ok())
                        .filter(|n| *n > 0)
                        .ok_or(ParseError("limit must be positive"))?,
                );
            }
            "skip" if self.skip == 0 => {
                self.skip = args[0].as_u64().ok_or(ParseError("invalid skip"))?;
            }
            _ => return Err(ParseError("unsupported cursor chain")),
        }
        Ok(())
    }

    /// Read/write classification used by the shared safety policy.
    #[must_use]
    pub fn classification(&self) -> Classification {
        let kind = match self.method.as_str() {
            "getIndexes" => StatementKind::Read,
            "find" | "countDocuments" | "aggregate"
                if !self.arguments.iter().any(writes_pipeline) =>
            {
                StatementKind::Read
            }
            "createIndex" | "dropIndex" => StatementKind::Ddl,
            _ => StatementKind::Write,
        };
        let unbounded = matches!(self.method.as_str(), "deleteMany" | "updateMany")
            && self
                .arguments
                .first()
                .and_then(Value::as_object)
                .is_some_and(serde_json::Map::is_empty);
        Classification {
            kind,
            dangerous: unbounded || self.method == "dropIndex",
        }
    }
}

fn writes_pipeline(value: &Value) -> bool {
    match value {
        Value::Object(fields) => fields.iter().any(|(key, value)| {
            matches!(
                key.as_str(),
                "$out" | "$merge" | "$function" | "$accumulator" | "$where"
            ) || writes_pipeline(value)
        }),
        Value::Array(values) => values.iter().any(writes_pipeline),
        _ => false,
    }
}

/// Classify malformed input conservatively as unknown.
#[must_use]
pub fn classify(source: &str) -> Classification {
    parse(source).map_or(
        Classification {
            kind: StatementKind::Unknown,
            dangerous: false,
        },
        |q| q.classification(),
    )
}
