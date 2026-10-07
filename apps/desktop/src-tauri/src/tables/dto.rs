// SPDX-License-Identifier: Apache-2.0
//! Request and reply types of the table-view commands.
use openrows_driver_api::catalog::{ColumnSummary, IndexSummary, TableStructure};
use openrows_sql::select::{
    Combinator, Filter, FilterOp, Sort, SortDirection, TableQuery, TableRef,
};
use serde::{Deserialize, Serialize};
use specta::Type;

/// The browsed relation on a saved connection's database.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TableTarget {
    pub(crate) profile_id: String,
    pub(crate) database: String,
    pub(crate) schema: String,
    pub(crate) table: String,
}

/// Filter-row comparisons (FR-GRID-11 subset).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum FilterOperatorDto {
    Equal,
    NotEqual,
    Less,
    Greater,
    LessOrEqual,
    GreaterOrEqual,
    Contains,
    NotContains,
    StartsWith,
    EndsWith,
    In,
    IsNull,
    IsNotNull,
    Between,
    NotIn,
    NotBetween,
    Like,
    NotLike,
    Regex,
    NotRegex,
    IsEmpty,
    IsNotEmpty,
}

/// One enabled filter row; `second` is BETWEEN's upper bound.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FilterDto {
    pub(crate) column: String,
    pub(crate) operator: FilterOperatorDto,
    pub(crate) value: String,
    pub(crate) second: String,
}

/// Whether every filter or any filter must match.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CombinatorDto {
    And,
    Or,
}

/// Server-side sort on one column.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SortDto {
    pub(crate) column: String,
    pub(crate) descending: bool,
}

/// What a table page shows: projection (empty = every column), filters, sort.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TableQueryDto {
    pub(crate) target: TableTarget,
    pub(crate) columns: Vec<String>,
    pub(crate) filters: Vec<FilterDto>,
    pub(crate) combinator: CombinatorDto,
    pub(crate) sort: Option<SortDto>,
}

/// One page of rows on an open session.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PageRequest {
    pub(crate) session_id: String,
    pub(crate) query: TableQueryDto,
    pub(crate) offset: u32,
    pub(crate) limit: u32,
}

/// An exact, cancellable count; `ticket` is the id `query_cancel` takes.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CountRequest {
    pub(crate) session_id: String,
    pub(crate) query: TableQueryDto,
    pub(crate) ticket: String,
}

/// A started page: cancel with `query_cancel(queryId)`; events follow.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PageReply {
    pub(crate) query_id: String,
    pub(crate) sql: String,
}

/// How an exact count ended.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum CountReply {
    Counted { rows: f64 },
    Cancelled,
}

/// Generated SQL and its bind parameters, for the SQL preview.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SqlPreviewDto {
    pub(crate) sql: String,
    pub(crate) params: Vec<Option<String>>,
}

/// A column on the Structure tab.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TableColumnDto {
    name: String,
    data_type: String,
    nullable: bool,
    default: Option<String>,
    comment: Option<String>,
}

/// An index on the Indexes tab.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TableIndexDto {
    name: String,
    columns: String,
    unique: bool,
    primary: bool,
    method: String,
    predicate: Option<String>,
}

/// Columns, indexes and the planner row estimate of a relation.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TableStructureDto {
    columns: Vec<TableColumnDto>,
    indexes: Vec<TableIndexDto>,
    row_estimate: Option<f64>,
}

impl From<ColumnSummary> for TableColumnDto {
    fn from(column: ColumnSummary) -> Self {
        Self {
            name: column.name,
            data_type: column.data_type,
            nullable: column.nullable,
            default: column.default,
            comment: column.comment,
        }
    }
}

impl From<IndexSummary> for TableIndexDto {
    fn from(index: IndexSummary) -> Self {
        Self {
            name: index.name,
            columns: index.columns,
            unique: index.unique,
            primary: index.primary,
            method: index.method,
            predicate: index.predicate,
        }
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "planner estimates are approximate and far below 2^53"
)]
impl From<TableStructure> for TableStructureDto {
    fn from(structure: TableStructure) -> Self {
        Self {
            columns: structure.columns.into_iter().map(Into::into).collect(),
            indexes: structure.indexes.into_iter().map(Into::into).collect(),
            row_estimate: structure.row_estimate.map(|rows| rows as f64),
        }
    }
}

impl From<FilterOperatorDto> for FilterOp {
    fn from(operator: FilterOperatorDto) -> Self {
        match operator {
            FilterOperatorDto::Equal => Self::Equal,
            FilterOperatorDto::NotEqual => Self::NotEqual,
            FilterOperatorDto::Less => Self::Less,
            FilterOperatorDto::Greater => Self::Greater,
            FilterOperatorDto::LessOrEqual => Self::LessOrEqual,
            FilterOperatorDto::GreaterOrEqual => Self::GreaterOrEqual,
            FilterOperatorDto::Contains => Self::Contains,
            FilterOperatorDto::NotContains => Self::NotContains,
            FilterOperatorDto::StartsWith => Self::StartsWith,
            FilterOperatorDto::EndsWith => Self::EndsWith,
            FilterOperatorDto::In => Self::In,
            FilterOperatorDto::IsNull => Self::IsNull,
            FilterOperatorDto::IsNotNull => Self::IsNotNull,
            FilterOperatorDto::Between => Self::Between,
            FilterOperatorDto::NotIn => Self::NotIn,
            FilterOperatorDto::NotBetween => Self::NotBetween,
            FilterOperatorDto::Like => Self::Like,
            FilterOperatorDto::NotLike => Self::NotLike,
            FilterOperatorDto::Regex => Self::Regex,
            FilterOperatorDto::NotRegex => Self::NotRegex,
            FilterOperatorDto::IsEmpty => Self::IsEmpty,
            FilterOperatorDto::IsNotEmpty => Self::IsNotEmpty,
        }
    }
}

impl TableQueryDto {
    /// The pure SQL-generation model of this request.
    pub(crate) fn to_query(&self) -> TableQuery {
        TableQuery {
            table: TableRef {
                schema: self.target.schema.clone(),
                name: self.target.table.clone(),
            },
            columns: self.columns.clone(),
            filters: self
                .filters
                .iter()
                .map(|filter| Filter {
                    column: filter.column.clone(),
                    op: filter.operator.into(),
                    value: filter.value.clone(),
                    second: filter.second.clone(),
                })
                .collect(),
            combinator: match self.combinator {
                CombinatorDto::And => Combinator::And,
                CombinatorDto::Or => Combinator::Or,
            },
            sort: self.sort.as_ref().map(|sort| Sort {
                column: sort.column.clone(),
                direction: if sort.descending {
                    SortDirection::Descending
                } else {
                    SortDirection::Ascending
                },
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_deserialize_from_camel_case() -> Result<(), serde_json::Error> {
        let request: PageRequest = serde_json::from_value(serde_json::json!({
            "sessionId": "s",
            "offset": 300,
            "limit": 300,
            "query": {
                "target": { "profileId": "p", "database": "d", "schema": "public", "table": "users" },
                "columns": ["id"],
                "filters": [{ "column": "name", "operator": "notContains", "value": "x", "second": "" }],
                "combinator": "or",
                "sort": { "column": "id", "descending": true }
            }
        }))?;
        let query = request.query.to_query();
        assert_eq!(query.table.name, "users");
        assert_eq!(query.combinator, Combinator::Or);
        assert_eq!(
            query.filters.first().map(|f| f.op),
            Some(FilterOp::NotContains)
        );
        assert_eq!(
            query.sort.map(|sort| sort.direction),
            Some(SortDirection::Descending)
        );
        Ok(())
    }

    #[test]
    fn replies_serialize_with_tags() -> Result<(), serde_json::Error> {
        let counted = serde_json::to_value(CountReply::Counted { rows: 1811.0 })?;
        assert_eq!(counted["kind"], "counted");
        assert_eq!(counted["rows"], 1811.0);
        let structure = TableStructureDto::from(TableStructure {
            row_estimate: Some(7),
            ..TableStructure::default()
        });
        assert_eq!(serde_json::to_value(structure)?["rowEstimate"], 7.0);
        Ok(())
    }
}
