// SPDX-License-Identifier: Apache-2.0
//! Query parameters of an editor run (Settings ▸ Query Params): the chosen
//! placeholder style is rewritten into the engine's bind placeholders and the
//! values travel as bound parameters, never as SQL text.
use openrows_sql::select::BoundSql;
use openrows_sql::{Dialect, ParamBinder, ParamSlot, ParamStyle};
use serde::Deserialize;
use specta::Type;

/// Which placeholders the editor treats as parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ParamStyleDto {
    /// `:name`.
    Named,
    /// `$1` (`PostgreSQL`), `?1` (`SQLite`).
    Numbered,
    /// `?`, asked for as `?1`, `?2`, … across the run.
    Anonymous,
}

impl From<ParamStyleDto> for ParamStyle {
    fn from(style: ParamStyleDto) -> Self {
        match style {
            ParamStyleDto::Named => Self::Named,
            ParamStyleDto::Numbered => Self::Numbered,
            ParamStyleDto::Anonymous => Self::Anonymous,
        }
    }
}

/// A value the user entered for one parameter name; `null` binds SQL NULL.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ParamValueDto {
    pub(crate) name: String,
    pub(crate) value: Option<String>,
}

/// Parameters of a run; `values` is `null` until the user entered them.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ParamsDto {
    pub(crate) style: ParamStyleDto,
    pub(crate) values: Option<Vec<ParamValueDto>>,
}

/// Statements ready to run, or the parameter names still needing values.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Binding {
    Ready(Vec<BoundSql>),
    Ask(Vec<String>),
}

/// Bind the run's statements; without parameters they run as written.
pub(crate) fn bind(statements: &[String], dialect: Dialect, params: Option<&ParamsDto>) -> Binding {
    let plain = || {
        let bound = statements.iter().map(|sql| BoundSql {
            sql: sql.clone(),
            params: Vec::new(),
        });
        Binding::Ready(bound.collect())
    };
    let Some(params) = params else {
        return plain();
    };
    let mut binder = ParamBinder::new(dialect, params.style.into());
    let slots: Vec<Vec<ParamSlot>> = statements.iter().map(|sql| binder.slots(sql)).collect();
    if slots.iter().all(Vec::is_empty) {
        return plain();
    }
    let Some(values) = &params.values else {
        return Binding::Ask(names(&slots));
    };
    let value = |name: &str| {
        let found = values.iter().find(|value| value.name == name);
        found.map(|value| value.value.clone())
    };
    statements
        .iter()
        .zip(&slots)
        .map(|(sql, slots)| binder.bind(sql, slots, value))
        .collect::<Result<Vec<_>, _>>()
        .map_or_else(|_| Binding::Ask(names(&slots)), Binding::Ready)
}

/// Distinct parameter names in order of first use.
fn names(slots: &[Vec<ParamSlot>]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for slot in slots.iter().flatten() {
        if !names.contains(&slot.name) {
            names.push(slot.name.clone());
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(values: Option<&[(&str, Option<&str>)]>) -> ParamsDto {
        ParamsDto {
            style: ParamStyleDto::Named,
            values: values.map(|pairs| {
                pairs
                    .iter()
                    .map(|(name, value)| ParamValueDto {
                        name: (*name).to_owned(),
                        value: value.map(str::to_owned),
                    })
                    .collect()
            }),
        }
    }

    fn statements(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| (*item).to_owned()).collect()
    }

    #[test]
    fn statements_without_placeholders_run_as_written() {
        let sql = statements(&["SELECT 1"]);
        let plain = Binding::Ready(vec![BoundSql {
            sql: "SELECT 1".to_owned(),
            params: Vec::new(),
        }]);
        assert_eq!(bind(&sql, Dialect::Postgres, None), plain);
        assert_eq!(bind(&sql, Dialect::Postgres, Some(&params(None))), plain);
    }

    #[test]
    fn missing_values_ask_for_every_name_once() {
        let sql = statements(&["SELECT :a, :b", "SELECT :a"]);
        let ask = Binding::Ask(vec![":a".to_owned(), ":b".to_owned()]);
        assert_eq!(bind(&sql, Dialect::Postgres, Some(&params(None))), ask);
        let partial = params(Some(&[(":a", Some("1"))]));
        assert_eq!(bind(&sql, Dialect::Postgres, Some(&partial)), ask);
    }

    #[test]
    fn values_are_bound_never_spliced() {
        let sql = statements(&["SELECT :a", "SELECT :a, :b"]);
        let values = params(Some(&[(":a", Some("1'; DROP TABLE t; --")), (":b", None)]));
        let bound = match bind(&sql, Dialect::Postgres, Some(&values)) {
            Binding::Ready(bound) => bound,
            Binding::Ask(_) => Vec::new(),
        };
        assert_eq!(bound.len(), 2);
        assert_eq!(bound[1].sql, "SELECT $1, $2");
        assert_eq!(
            bound[1].params,
            vec![Some("1'; DROP TABLE t; --".to_owned()), None]
        );
        assert!(!bound[0].sql.contains("DROP"));
    }
}
