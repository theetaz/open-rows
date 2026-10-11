// SPDX-License-Identifier: Apache-2.0
//! Traverse every branch, tracking CTE scope and live base-table validation inputs.
use super::{UnsafeAiSql, expressions};
use sqlparser::ast::{
    Expr, Query, Select, SelectItem, SetExpr, Statement, TableFactor, Visit, Visitor,
};
use std::ops::ControlFlow;
#[derive(Default)]
pub(super) struct Inspection {
    pub(super) dialect: crate::Dialect,
    pub(super) sources: Vec<(String, String)>,
    pub(super) scopes: Vec<Scope>,
}
pub(super) struct Scope {
    visible: Vec<String>,
    definitions: Vec<(usize, String)>,
}
impl Visitor for Inspection {
    type Break = UnsafeAiSql;
    fn pre_visit_statement(&mut self, statement: &Statement) -> ControlFlow<Self::Break> {
        check(matches!(statement, Statement::Query(_)))
    }
    fn pre_visit_query(&mut self, query: &Query) -> ControlFlow<Self::Break> {
        check(
            query.locks.is_empty()
                && query.for_clause.is_none()
                && query.settings.is_none()
                && query.format_clause.is_none()
                && query.pipe_operators.is_empty()
                && body(&query.body),
        )?;
        if query.with.as_ref().is_some_and(|with| with.recursive) {
            return ControlFlow::Break(UnsafeAiSql);
        }
        let Some(definitions) = cte_definitions(query, self.dialect) else {
            return ControlFlow::Break(UnsafeAiSql);
        };
        self.scopes.push(Scope {
            visible: vec![],
            definitions,
        });
        ControlFlow::Continue(())
    }
    fn post_visit_query(&mut self, query: &Query) -> ControlFlow<Self::Break> {
        self.scopes.pop();
        // A non-recursive definition sees earlier CTEs, never itself or later CTEs.
        // Otherwise an unqualified base table/view can hide behind a future alias.
        if let Some(parent) = self.scopes.last_mut()
            && let Some((_, name)) = parent
                .definitions
                .iter()
                .find(|(address, _)| *address == std::ptr::from_ref(query).addr())
        {
            parent.visible.push(name.clone());
        }
        ControlFlow::Continue(())
    }
    fn pre_visit_select(&mut self, select: &Select) -> ControlFlow<Self::Break> {
        check(
            select.into.is_none()
                && select.optimizer_hints.is_empty()
                && select.select_modifiers.is_none()
                && select.top.is_none()
                && select.lateral_views.is_empty()
                && select.connect_by.is_empty()
                && !select.projection.iter().any(|item| {
                    matches!(
                        item,
                        SelectItem::ExprWithAlias {
                            expr: Expr::Identifier(_) | Expr::CompoundIdentifier(_),
                            ..
                        } | SelectItem::ExprWithAliases { .. }
                    )
                }),
        )
    }
    fn pre_visit_expr(&mut self, expr: &Expr) -> ControlFlow<Self::Break> {
        check(expressions::allowed(expr, self.dialect))
    }
    fn pre_visit_table_factor(&mut self, table: &TableFactor) -> ControlFlow<Self::Break> {
        match table {
            TableFactor::Table {
                name,
                args,
                with_hints,
                version,
                with_ordinality,
                partitions,
                json_path,
                sample,
                index_hints,
                ..
            } => {
                check(
                    args.is_none()
                        && with_hints.is_empty()
                        && version.is_none()
                        && !with_ordinality
                        && partitions.is_empty()
                        && json_path.is_none()
                        && sample.is_none()
                        && index_hints.is_empty(),
                )?;
                self.relation(name)
            }
            TableFactor::Derived {
                lateral: false,
                sample: None,
                ..
            }
            | TableFactor::NestedJoin { .. } => ControlFlow::Continue(()),
            _ => ControlFlow::Break(UnsafeAiSql),
        }
    }
}
impl Inspection {
    fn relation(&mut self, name: &sqlparser::ast::ObjectName) -> ControlFlow<UnsafeAiSql> {
        let Some(parts) = expressions::relation_parts(name, self.dialect) else {
            return ControlFlow::Break(UnsafeAiSql);
        };
        match parts.as_slice() {
            [cte]
                if self
                    .scopes
                    .iter()
                    .rev()
                    .any(|scope| scope.visible.contains(cte)) => {}
            [schema, table] => {
                let source = (schema.clone(), table.clone());
                if !self.sources.contains(&source) {
                    self.sources.push(source);
                }
            }
            _ => return ControlFlow::Break(UnsafeAiSql),
        }
        check(self.sources.len() <= 32)
    }
}
fn cte_definitions(query: &Query, dialect: crate::Dialect) -> Option<Vec<(usize, String)>> {
    query.with.as_ref().map_or_else(
        || Some(vec![]),
        |with| {
            with.cte_tables
                .iter()
                .map(|cte| {
                    Some((
                        std::ptr::from_ref(cte.query.as_ref()).addr(),
                        expressions::relation_ident(&cte.alias.name, dialect)?,
                    ))
                })
                .collect()
        },
    )
}
fn check(allowed: bool) -> ControlFlow<UnsafeAiSql> {
    if allowed {
        ControlFlow::Continue(())
    } else {
        ControlFlow::Break(UnsafeAiSql)
    }
}
fn body(body: &SetExpr) -> bool {
    match body {
        SetExpr::Select(_) | SetExpr::Query(_) => true,
        SetExpr::SetOperation { left, right, .. } => self::body(left) && self::body(right),
        _ => false,
    }
}
pub(super) fn simple_source(query: &Query, dialect: crate::Dialect) -> Option<(String, String)> {
    if query.with.is_some() {
        return None;
    }
    let SetExpr::Select(select) = query.body.as_ref() else {
        return None;
    };
    let [from] = select.from.as_slice() else {
        return None;
    };
    if !from.joins.is_empty()
        || !select.projection.iter().all(|item| {
            matches!(
                item,
                SelectItem::UnnamedExpr(Expr::Identifier(_) | Expr::CompoundIdentifier(_))
                    | SelectItem::Wildcard(_)
                    | SelectItem::QualifiedWildcard(_, _)
            )
        })
    {
        return None;
    }
    let TableFactor::Table {
        name, alias: None, ..
    } = &from.relation
    else {
        return None;
    };
    let mut visitor = Inspection {
        dialect,
        ..Inspection::default()
    };
    if query.visit(&mut visitor).is_break() || visitor.sources.len() != 1 {
        return None;
    }
    let parts = expressions::relation_parts(name, dialect)?;
    Some((parts.first()?.clone(), parts.get(1)?.clone()))
}
