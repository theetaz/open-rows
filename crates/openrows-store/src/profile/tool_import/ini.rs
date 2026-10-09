// SPDX-License-Identifier: Apache-2.0
//! Small public INI reader; ignores includes instead of following arbitrary paths.
use crate::profile::exchange::ImportBatch;
use std::collections::BTreeMap;
use zeroize::Zeroizing;
#[derive(Default)]
pub(super) struct Section {
    pub name: String,
    pub values: BTreeMap<String, Zeroizing<String>>,
}
pub(super) fn sections(text: &str, mysql: bool, batch: &mut ImportBatch) -> Vec<Section> {
    let mut sections: Vec<Section> = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(['#', ';']) {
            continue;
        }
        if line.starts_with('!') {
            batch
                .warnings
                .push(format!("line {}: include directive ignored", index + 1));
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            sections.push(Section {
                name: line[1..line.len() - 1].trim().to_owned(),
                ..Section::default()
            });
        } else if let Some(section) = sections.last_mut() {
            let (key, value) = line.split_once('=').unwrap_or((line, ""));
            let value = if mysql {
                mysql_value(value.trim())
            } else {
                Zeroizing::new(value.trim().to_owned())
            };
            section.values.insert(
                key.trim().replace('_', if mysql { "-" } else { "_" }),
                value,
            );
        } else {
            batch.warnings.push(format!(
                "line {}: option outside a group ignored",
                index + 1
            ));
        }
    }
    sections
}
fn mysql_value(value: &str) -> Zeroizing<String> {
    let value = if value.starts_with(['\'', '"']) {
        value
    } else {
        value.split('#').next().unwrap_or_default().trim_end()
    };
    let mut decoded = Zeroizing::new(String::with_capacity(value.len()));
    let mut chars = value.chars().peekable();
    let quote = chars.peek().copied().filter(|c| matches!(c, '\'' | '"'));
    if quote.is_some() {
        chars.next();
    }
    while let Some(character) = chars.next() {
        if Some(character) == quote {
            break;
        }
        if quote.is_none() && character == '#' {
            break;
        }
        if character == '\\' {
            match chars.next() {
                Some('n') => decoded.push('\n'),
                Some('r') => decoded.push('\r'),
                Some('t') => decoded.push('\t'),
                Some('b') => decoded.push('\u{8}'),
                Some('s') => decoded.push(' '),
                Some('\\') | None => decoded.push('\\'),
                Some(c @ ('\'' | '"')) => decoded.push(c),
                Some(c) => {
                    decoded.push('\\');
                    decoded.push(c);
                }
            }
        } else {
            decoded.push(character);
        }
    }

    decoded
}
