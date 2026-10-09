// SPDX-License-Identifier: Apache-2.0
//! Reading plan JSON leniently: engines write numbers as numbers or as
//! quoted text (`MySQL` costs), and unknown values become detail text.
use serde_json::{Map, Value};

/// A JSON object.
pub(crate) type Object = Map<String, Value>;

/// A number written as a JSON number or as numeric text.
pub(crate) fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

/// `object[key]` as a number.
pub(crate) fn field_number(object: &Object, key: &str) -> Option<f64> {
    object.get(key).and_then(number)
}

/// `object[key]` as display text; `None` when absent, null or empty.
pub(crate) fn field_text(object: &Object, key: &str) -> Option<String> {
    object.get(key).map(text).filter(|value| !value.is_empty())
}

/// Display text of any value: strings as written, arrays joined with `, `,
/// objects as compact JSON.
pub(crate) fn text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(text) => text.clone(),
        Value::Array(items) => items.iter().map(text).collect::<Vec<_>>().join(", "),
        Value::Bool(_) | Value::Number(_) | Value::Object(_) => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn numbers_may_be_quoted_and_text_flattens_arrays() {
        assert_eq!(number(&json!("12.5")), Some(12.5));
        assert_eq!(number(&json!(3)), Some(3.0));
        assert_eq!(number(&json!("39K")), None);
        assert_eq!(text(&json!(["a", 1, true])), "a, 1, true");
        assert_eq!(text(&json!({"a": 1})), r#"{"a":1}"#);
        let object = json!({"empty": "", "null": null})
            .as_object()
            .cloned()
            .unwrap_or_default();
        assert_eq!(field_text(&object, "empty"), None);
        assert_eq!(field_text(&object, "null"), None);
    }
}
