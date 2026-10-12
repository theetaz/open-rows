// SPDX-License-Identifier: Apache-2.0
//! Positive expression/function policy; no user code or engine-control functions.
use sqlparser::ast::{
    BinaryOperator, Expr, Function, FunctionArguments, ObjectName, UnaryOperator, Value,
};

pub(super) fn allowed(expr: &Expr, dialect: crate::Dialect) -> bool {
    match expr {
        Expr::Identifier(_)
        | Expr::CompoundIdentifier(_)
        | Expr::Nested(_)
        | Expr::IsNull(_)
        | Expr::IsNotNull(_)
        | Expr::IsTrue(_)
        | Expr::IsFalse(_)
        | Expr::IsNotTrue(_)
        | Expr::IsNotFalse(_)
        | Expr::IsDistinctFrom(_, _)
        | Expr::IsNotDistinctFrom(_, _)
        | Expr::InList { .. }
        | Expr::InSubquery { .. }
        | Expr::Between { .. }
        | Expr::Exists { .. }
        | Expr::Subquery(_)
        | Expr::Case { .. }
        | Expr::Like { .. }
        | Expr::ILike { .. } => true,
        Expr::Value(value) => matches!(
            value.value,
            Value::Number(_, _)
                | Value::SingleQuotedString(_)
                | Value::DoubleQuotedString(_)
                | Value::Boolean(_)
                | Value::Null
        ),
        Expr::BinaryOp { op, .. } => matches!(
            op,
            BinaryOperator::Plus
                | BinaryOperator::Minus
                | BinaryOperator::Multiply
                | BinaryOperator::Divide
                | BinaryOperator::Modulo
                | BinaryOperator::Eq
                | BinaryOperator::NotEq
                | BinaryOperator::Lt
                | BinaryOperator::LtEq
                | BinaryOperator::Gt
                | BinaryOperator::GtEq
                | BinaryOperator::And
                | BinaryOperator::Or
        ),
        Expr::UnaryOp { op, .. } => matches!(
            op,
            UnaryOperator::Plus | UnaryOperator::Minus | UnaryOperator::Not
        ),
        Expr::Function(function) => safe_function(function, dialect),
        _ => false,
    }
}
fn safe_function(function: &Function, dialect: crate::Dialect) -> bool {
    if function.name.0.iter().any(|part| {
        part.as_ident()
            .is_none_or(|ident| ident.quote_style.is_some())
    }) {
        return false;
    }
    let Some(parts) = parts(&function.name) else {
        return false;
    };
    let name = match parts.as_slice() {
        [name] => name,
        [schema, name] if schema == "pg_catalog" && dialect == crate::Dialect::Postgres => name,
        _ => return false,
    };
    let name = name.to_ascii_lowercase();
    !function.uses_odbc_syntax
        && matches!(function.parameters, FunctionArguments::None)
        && (window_function(&name) && function.over.is_some()
            || [
                "count", "sum", "avg", "min", "max", "abs", "round", "lower", "upper", "length",
                "coalesce", "nullif",
            ]
            .contains(&name.as_str()))
}
fn window_function(name: &str) -> bool {
    [
        "row_number",
        "rank",
        "dense_rank",
        "lag",
        "lead",
        "first_value",
        "last_value",
        "nth_value",
        "ntile",
        "percent_rank",
        "cume_dist",
    ]
    .contains(&name)
}
pub(super) fn parts(name: &ObjectName) -> Option<Vec<String>> {
    name.0
        .iter()
        .map(|part| part.as_ident().map(|ident| ident.value.clone()))
        .collect()
}
pub(super) fn relation_parts(name: &ObjectName, dialect: crate::Dialect) -> Option<Vec<String>> {
    name.0
        .iter()
        .map(|part| relation_ident(part.as_ident()?, dialect))
        .collect()
}
pub(super) fn relation_ident(
    ident: &sqlparser::ast::Ident,
    dialect: crate::Dialect,
) -> Option<String> {
    match dialect {
        crate::Dialect::Postgres if ident.quote_style.is_none() => ident
            .value
            .is_ascii()
            .then(|| ident.value.to_ascii_lowercase()),
        crate::Dialect::Postgres | crate::Dialect::MySql | crate::Dialect::Sqlite => {
            Some(ident.value.clone())
        }
        crate::Dialect::MongoDb
        | crate::Dialect::Duckdb
        | crate::Dialect::SqlServer
        | crate::Dialect::ClickHouse => None,
    }
}
