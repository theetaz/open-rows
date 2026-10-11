// SPDX-License-Identifier: Apache-2.0
//! Compile bounded rules once and scrub all retained fields before serialization.
use super::{ContextBundle, Limits, RedactionRule, Refusal};
use openrows_sql::{Dialect, redact_identifiers, redact_literals};
use regex::{Regex, RegexBuilder};
const MARKER: &str = "‹redacted›";
const SENSITIVE: &str = "password|passwd|passphrase|token|secret|ssn|email|phone|card|api[_-]?key|private[_-]?key|credential";

pub(super) struct Redactor {
    rules: Vec<Regex>,
    limits: Limits,
    dialect: Dialect,
    namespaces: Vec<String>,
    pub(super) truncated: bool,
}
impl Redactor {
    pub(super) fn new(
        rules: &[RedactionRule],
        limits: Limits,
        dialect: Dialect,
    ) -> Result<Self, Refusal> {
        if rules.len() > 32 {
            return Err(Refusal::Invalid);
        }
        let mut compiled = vec![compile(SENSITIVE)?];
        for rule in rules {
            let pattern = match rule {
                RedactionRule::Glob(pattern) => glob(pattern),
                RedactionRule::Regex(pattern) => pattern.clone(),
            };
            if pattern.len() > 2048 {
                return Err(Refusal::Invalid);
            }
            compiled.push(compile(&pattern)?);
        }
        Ok(Self {
            rules: compiled,
            limits,
            dialect,
            namespaces: Vec::new(),
            truncated: false,
        })
    }
    pub(super) fn matches(&self, name: &str) -> bool {
        self.rules.iter().any(|rule| rule.is_match(name))
    }
    fn field(&mut self, value: &str) -> String {
        let end = value.floor_char_boundary(value.len().min(self.limits.field_bytes));
        self.truncated |= end < value.len();
        value[..end].into()
    }
    fn identifier(&mut self, name: &str) -> String {
        if self.matches(name) {
            MARKER.into()
        } else {
            self.field(name)
        }
    }
    fn sql(&mut self, sql: &str) -> String {
        let literals = redact_literals(sql, self.dialect);
        let output = redact_identifiers(&literals, self.dialect, |name| {
            self.matches(name)
                || self
                    .namespaces
                    .iter()
                    .any(|namespace| self.matches(&format!("{namespace}.{name}")))
        });
        self.field(&output)
    }
    pub(super) fn bundle(&mut self, mut bundle: ContextBundle) -> Result<ContextBundle, Refusal> {
        self.namespaces = bundle
            .schema
            .iter()
            .map(|object| object.name.clone())
            .chain(bundle.samples.iter().map(|sample| sample.table.clone()))
            .chain(bundle.plan.iter().map(|node| node.relation.clone()))
            .collect();
        for object in &mut bundle.schema {
            object.name = self.identifier(&object.name);
            object.ddl = self.sql(&object.ddl);
        }
        bundle.query = bundle.query.map(|query| self.sql(&query));
        for plan in &mut bundle.plan {
            plan.relation = self.identifier(&plan.relation);
            plan.predicate = self.sql(&plan.predicate);
        }
        for stat in &mut bundle.stats {
            stat.query = self.sql(&stat.query);
        }
        self.samples(&mut bundle)?;
        Ok(bundle)
    }
    fn samples(&mut self, bundle: &mut ContextBundle) -> Result<(), Refusal> {
        let (mut rows, mut cells) = (self.limits.rows, self.limits.cells);
        for sample in &mut bundle.samples {
            if sample.columns.is_empty()
                || sample
                    .rows
                    .iter()
                    .any(|row| row.len() != sample.columns.len())
            {
                return Err(Refusal::Invalid);
            }
            let keep = rows
                .min(cells / sample.columns.len())
                .min(sample.rows.len());
            self.truncated |= keep < sample.rows.len();
            sample.rows.truncate(keep);
            rows -= keep;
            cells -= keep * sample.columns.len();
            for (index, column) in sample.columns.iter_mut().enumerate() {
                let sensitive = self.matches(&format!("{}.{}", sample.table, column));
                self.values(&mut sample.rows, index, sensitive);
                *column = self.identifier(&format!("{}.{}", sample.table, column));
            }
            sample.table = self.identifier(&sample.table);
        }
        bundle.samples.retain(|sample| !sample.rows.is_empty());
        Ok(())
    }
    fn values(&mut self, rows: &mut [Vec<Option<String>>], index: usize, sensitive: bool) {
        for row in rows {
            row[index] = row[index]
                .as_ref()
                .map(|value| self.value(value, sensitive));
        }
    }
    fn value(&mut self, value: &str, sensitive: bool) -> String {
        if sensitive {
            MARKER.into()
        } else {
            self.field(value)
        }
    }
}
fn compile(pattern: &str) -> Result<Regex, Refusal> {
    RegexBuilder::new(pattern)
        .case_insensitive(true)
        .size_limit(128 * 1024)
        .dfa_size_limit(128 * 1024)
        .build()
        .map_err(|_| Refusal::Invalid)
}
fn glob(pattern: &str) -> String {
    let mut output = String::from("^");
    for character in pattern.chars() {
        match character {
            '*' => output.push_str(".*"),
            '?' => output.push('.'),
            other => output.push_str(&regex::escape(&other.to_string())),
        }
    }
    output.push('$');
    output
}
