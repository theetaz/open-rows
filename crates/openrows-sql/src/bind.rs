// SPDX-License-Identifier: Apache-2.0
//! Editor query parameters: find the placeholders of the user's chosen style
//! and rewrite them into the dialect's bind placeholders, so values always
//! travel as bound parameters and never as SQL text.

use std::fmt;
use std::ops::Range;

use crate::select::BoundSql;
use crate::{Dialect, PlaceholderKind, placeholders};

/// Placeholder syntax the user writes for query parameters.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ParamStyle {
    /// `:name` (also `SQLite` `$name` and `@name`).
    Named,
    /// `$1`, `$2`, … (`PostgreSQL`) or `?1` (`SQLite`).
    Numbered,
    /// `?`; each one is a separate parameter, named `?1`, `?2`, … across a run.
    Anonymous,
}

impl ParamStyle {
    fn matches(self, kind: PlaceholderKind) -> bool {
        matches!(
            (self, kind),
            (Self::Named, PlaceholderKind::Named)
                | (Self::Numbered, PlaceholderKind::Positional)
                | (Self::Anonymous, PlaceholderKind::Anonymous)
        )
    }
}

/// One parameter occurrence: where it is and the name its value is asked by.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParamSlot {
    /// Byte range of the placeholder, including its sigil.
    pub range: Range<usize>,
    /// `:id`, `$1` or `?1`; repeated names share one value.
    pub name: String,
}

/// A parameter without a value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MissingParam(pub String);

impl fmt::Display for MissingParam {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "no value for parameter {}", self.0)
    }
}

impl std::error::Error for MissingParam {}

/// Finds the parameters of a run's statements in order; anonymous `?`
/// placeholders are numbered across the whole run.
#[derive(Clone, Debug)]
pub struct ParamBinder {
    dialect: Dialect,
    style: ParamStyle,
    anonymous: usize,
}

impl ParamBinder {
    /// A binder for statements of `dialect` written with `style` placeholders.
    #[must_use]
    pub const fn new(dialect: Dialect, style: ParamStyle) -> Self {
        Self {
            dialect,
            style,
            anonymous: 0,
        }
    }

    /// Parameter occurrences of the run's next statement, in source order.
    pub fn slots(&mut self, sql: &str) -> Vec<ParamSlot> {
        placeholders(sql, self.dialect)
            .into_iter()
            .filter(|placeholder| self.style.matches(placeholder.kind))
            .filter_map(|placeholder| {
                let name = if self.style == ParamStyle::Anonymous {
                    self.anonymous += 1;
                    format!("?{}", self.anonymous)
                } else {
                    sql.get(placeholder.range.clone())?.to_owned()
                };
                Some(ParamSlot {
                    range: placeholder.range,
                    name,
                })
            })
            .collect()
    }

    fn placeholder(&self, index: usize, name: &str) -> String {
        if self.dialect == Dialect::ClickHouse
            && name.starts_with('{')
            && let Some((_, kind)) = name.split_once(':')
        {
            return format!("{{p{index}:{kind}");
        }
        self.dialect.placeholder(index)
    }

    /// `sql` with every slot replaced by the dialect's bind placeholder and
    /// the values in placeholder order. `value` returns `None` for a name
    /// without a value and `Some(None)` for SQL NULL.
    ///
    /// # Errors
    /// [`MissingParam`] naming the first slot `value` has no value for.
    pub fn bind(
        &self,
        sql: &str,
        slots: &[ParamSlot],
        value: impl Fn(&str) -> Option<Option<String>>,
    ) -> Result<BoundSql, MissingParam> {
        let mut text = String::with_capacity(sql.len());
        let mut params = Vec::with_capacity(slots.len());
        let mut copied = 0;
        for (index, slot) in slots.iter().enumerate() {
            let value = value(&slot.name).ok_or_else(|| MissingParam(slot.name.clone()))?;
            text.push_str(sql.get(copied..slot.range.start).unwrap_or_default());
            text.push_str(&self.placeholder(index + 1, &slot.name));
            params.push(value);
            copied = slot.range.end;
        }
        text.push_str(sql.get(copied..).unwrap_or_default());
        Ok(BoundSql { sql: text, params })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(name: &str) -> String {
        format!("<{name}>")
    }

    #[test]
    fn named_parameters_become_dialect_placeholders() -> Result<(), MissingParam> {
        let sql = "SELECT * FROM t WHERE a = :id AND b::text = :name OR c = :id";
        let mut binder = ParamBinder::new(Dialect::Postgres, ParamStyle::Named);
        let slots = binder.slots(sql);
        let names: Vec<&str> = slots.iter().map(|slot| slot.name.as_str()).collect();
        assert_eq!(names, [":id", ":name", ":id"]);
        let bound = binder.bind(sql, &slots, |name| Some(Some(value(name))))?;
        assert_eq!(
            bound.sql,
            "SELECT * FROM t WHERE a = $1 AND b::text = $2 OR c = $3"
        );
        assert_eq!(
            bound.params,
            [Some("<:id>"), Some("<:name>"), Some("<:id>")].map(|v| v.map(str::to_owned))
        );
        let mysql = ParamBinder::new(Dialect::MySql, ParamStyle::Named);
        let bound = mysql.bind("x = :a", &mysql.clone().slots("x = :a"), |name| {
            Some(Some(value(name)))
        })?;
        assert_eq!(bound.sql, "x = ?");
        Ok(())
    }

    #[test]
    fn numbered_parameters_are_reordered_by_occurrence() -> Result<(), MissingParam> {
        let sql = "SELECT $2, '$1', $1";
        let mut binder = ParamBinder::new(Dialect::Postgres, ParamStyle::Numbered);
        let slots = binder.slots(sql);
        let bound = binder.bind(sql, &slots, |name| Some(Some(value(name))))?;
        assert_eq!(bound.sql, "SELECT $1, '$1', $2");
        assert_eq!(
            bound.params,
            [Some("<$2>"), Some("<$1>")].map(|v| v.map(str::to_owned))
        );
        Ok(())
    }

    #[test]
    fn anonymous_parameters_are_numbered_across_statements() {
        let mut binder = ParamBinder::new(Dialect::MySql, ParamStyle::Anonymous);
        let first: Vec<String> = binder
            .slots("a = ? AND b = '?'")
            .into_iter()
            .map(|s| s.name)
            .collect();
        let second: Vec<String> = binder.slots("c = ?").into_iter().map(|s| s.name).collect();
        assert_eq!(first, ["?1"]);
        assert_eq!(second, ["?2"]);
    }

    #[test]
    fn null_values_bind_as_sql_null() -> Result<(), MissingParam> {
        let sql = "SELECT :a";
        let mut binder = ParamBinder::new(Dialect::Postgres, ParamStyle::Named);
        let slots = binder.slots(sql);
        let bound = binder.bind(sql, &slots, |_| Some(None))?;
        assert_eq!(bound.params, [None]);
        Ok(())
    }

    #[test]
    fn other_styles_and_missing_values_are_reported() {
        let mut binder = ParamBinder::new(Dialect::Postgres, ParamStyle::Named);
        assert!(binder.slots("SELECT $1, ?").is_empty());
        let slots = binder.slots("SELECT :a");
        assert_eq!(
            binder.bind("SELECT :a", &slots, |_| None),
            Err(MissingParam(":a".to_owned()))
        );
    }
}
