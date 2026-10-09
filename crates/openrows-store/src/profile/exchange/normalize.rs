// SPDX-License-Identifier: Apache-2.0
//! Schema validation and recursive unknown-field projection with value-free warnings.
use super::{CONNECTIONS_SCHEMA, ExchangeError};
use serde_json::Value;
fn schema() -> Result<Value, ExchangeError> {
    serde_json::from_str(CONNECTIONS_SCHEMA)
        .map_err(|_| ExchangeError::Invalid("invalid bundled schema".into()))
}
pub(super) fn validate(value: &Value) -> Result<(), ExchangeError> {
    let validator = jsonschema::validator_for(&schema()?)
        .map_err(|_| ExchangeError::Invalid("invalid bundled schema".into()))?;
    if let Some(error) = validator.iter_errors(value).next() {
        // ValidationError's display can contain input values, so only expose its pointer.
        return Err(ExchangeError::Invalid(format!(
            "check field {}",
            error.instance_path
        )));
    }
    Ok(())
}
pub(super) fn unknown_fields(
    value: &mut Value,
    warnings: &mut Vec<String>,
) -> Result<(), ExchangeError> {
    let schema = schema()?;
    project(value, &schema, &schema, "", warnings);
    Ok(())
}
fn project(value: &mut Value, rule: &Value, root: &Value, path: &str, warnings: &mut Vec<String>) {
    let rule = rule
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(|reference| root.pointer(reference.trim_start_matches('#')))
        .unwrap_or(rule);
    if let (Some(object), Some(properties)) = (
        value.as_object_mut(),
        rule.get("properties").and_then(Value::as_object),
    ) {
        object.retain(|key, value| {
            let pointer = format!("{path}/{key}");
            let Some(next) = properties.get(key) else {
                warnings.push(format!("Ignored unknown field {pointer}"));
                return false;
            };
            project(value, next, root, &pointer, warnings);
            true
        });
    } else if let Some(items) = value.as_array_mut() {
        for (index, item) in items.iter_mut().enumerate() {
            project(
                item,
                &rule["items"],
                root,
                &format!("{path}/{index}"),
                warnings,
            );
        }
    }
}
