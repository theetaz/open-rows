// SPDX-License-Identifier: Apache-2.0
//! Arrow projection for types the `ClickHouse` LTS exporter cannot encode.
use openrows_driver_api::DriverError;
use openrows_sql::Dialect;
pub(crate) fn arrow_sql(sql: &str, types: &[(String, String)]) -> Result<String, DriverError> {
    if types.iter().all(|(_, kind)| arrow_type(kind) == *kind) {
        return Ok(format!("{sql}\nFORMAT ArrowStream"));
    }
    let mut fields = Vec::with_capacity(types.len());
    for (name, kind) in types {
        let quoted = Dialect::ClickHouse
            .quote_ident(name)
            .map_err(|e| DriverError::Protocol(e.to_string()))?;
        let target = arrow_type(kind);
        let field = if target == *kind {
            quoted
        } else {
            // The type comes from DESCRIBE, never from a caller or a bound value.
            let target = openrows_sql::ddl::string_literal(Dialect::ClickHouse, &target)
                .ok_or_else(|| {
                    DriverError::Protocol("invalid ClickHouse source type".to_owned())
                })?;
            format!("CAST({quoted}, {target}) AS {quoted}")
        };
        fields.push(field);
    }
    Ok(format!(
        "SELECT {} FROM ({sql}) FORMAT ArrowStream",
        fields.join(", ")
    ))
}

fn arrow_type(kind: &str) -> String {
    let mut output = String::with_capacity(kind.len());
    let mut quoted = false;
    let mut escaped = false;
    let mut word = String::new();
    for character in kind.chars() {
        if !quoted && (character.is_alphanumeric() || character == '_') {
            word.push(character);
            continue;
        }
        output.push_str(if word == "UUID" { "String" } else { &word });
        word.clear();
        output.push(character);
        if character == '\'' && !escaped {
            quoted = !quoted;
        }
        escaped = character == '\\' && !escaped;
    }
    output.push_str(if word == "UUID" { "String" } else { &word });
    output
}

/// Leading comments do not change the statement's command keyword.
pub(crate) fn verb(mut sql: &str) -> String {
    loop {
        sql = sql.trim_start();
        if sql.starts_with("--") || sql.starts_with('#') {
            sql = sql.split_once('\n').map_or("", |(_, tail)| tail);
        } else if sql.starts_with("/*") {
            sql = sql.split_once("*/").map_or("", |(_, tail)| tail);
        } else {
            return sql
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .to_ascii_uppercase();
        }
    }
}
