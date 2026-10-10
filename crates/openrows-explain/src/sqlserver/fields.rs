// SPDX-License-Identifier: Apache-2.0
//! Native showplan attributes as finite measures and object names.
use super::Attributes;
use crate::NodeData;

pub(super) fn number(attrs: &Attributes, name: &str) -> Option<f64> {
    attrs
        .get(name)?
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value >= 0.0)
}

pub(super) fn describe(attrs: &Attributes) -> String {
    attrs
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn object(data: &mut NodeData, attrs: &Attributes) {
    if let Some(table) = attrs.get("Table") {
        let schema = attrs
            .get("Schema")
            .map(|value| format!("{}.", unquote(value)))
            .unwrap_or_default();
        data.relation = Some(format!("{schema}{}", unquote(table)));
    }
    if let Some(index) = attrs.get("Index") {
        data.index = Some(unquote(index));
    }
}

fn unquote(value: &str) -> String {
    value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(value)
        .replace("]]", "]")
}
