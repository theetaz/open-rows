// SPDX-License-Identifier: Apache-2.0
//! A finite set of BSON constructors expands to EJSON data, never executable code.
use super::{ParseError, scan};
use serde_json::{Value, json};

pub(super) fn expand(source: &str) -> Result<String, ParseError> {
    let mut output = String::with_capacity(source.len());
    let mut rest = source;
    while !rest.is_empty() {
        let first = rest.chars().next().ok_or(ParseError("invalid argument"))?;
        if matches!(first, '\'' | '"') {
            let end = string_end(rest, first)?;
            output.push_str(&rest[..end]);
            rest = &rest[end..];
        } else if let Some((name, key)) = constructor(rest) {
            let tail = &rest[name.len() + 1..];
            let end = scan::closing(tail)?;
            let value = super::value::literal(tail[..end].trim())?;
            let value = match value {
                Value::String(value) => value,
                Value::Number(number) => number.to_string(),
                _ => return Err(ParseError("invalid BSON constructor")),
            };
            output.push_str(&json!({key:value}).to_string());
            rest = &tail[end + 1..];
        } else {
            output.push(first);
            rest = &rest[first.len_utf8()..];
        }
    }
    Ok(output)
}
fn constructor(source: &str) -> Option<(&'static str, &'static str)> {
    [
        ("ObjectId", "$oid"),
        ("ISODate", "$date"),
        ("NumberLong", "$numberLong"),
        ("NumberInt", "$numberInt"),
        ("NumberDecimal", "$numberDecimal"),
    ]
    .into_iter()
    .find(|(name, _)| {
        source
            .strip_prefix(name)
            .is_some_and(|rest| rest.starts_with('('))
    })
}
fn string_end(source: &str, quote: char) -> Result<usize, ParseError> {
    let mut escaped = false;
    for (offset, ch) in source.char_indices().skip(1) {
        if escaped {
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == quote {
            return Ok(offset + ch.len_utf8());
        }
    }
    Err(ParseError("unclosed string"))
}
