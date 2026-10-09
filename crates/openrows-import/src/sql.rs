// SPDX-License-Identifier: Apache-2.0
//! The SQL an import runs, in the target's dialect: multi-row parameterised
//! `INSERT`s with the conflict clause, the existing-key probe of the dry
//! run, `CREATE TABLE` for a new table, truncation and transaction control.
//! Every name is quoted; every value is a bind parameter.
use openrows_sql::Dialect;
use openrows_sql::ddl::valid_type;

use crate::ImportError;

/// A table, by schema (database for `MySQL`, attached file for `SQLite`) and name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TableName {
    /// Schema, database or attached file name.
    pub schema: String,
    /// Table name.
    pub table: String,
}

/// What happens to a row whose key already exists.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConflictPolicy {
    /// The server rejects it; the import fails.
    Error,
    /// The row is skipped (`ON CONFLICT DO NOTHING`).
    Skip,
    /// The existing row is updated with the row's values (upsert).
    Update,
}

/// A column of a table the import creates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewColumn {
    /// Column name.
    pub name: String,
    /// Column type in the target dialect, e.g. `bigint` or `VARCHAR(255)`.
    pub data_type: String,
}

/// Most bind parameters one statement may carry (SQL Server: 2 100 per
/// `sp_executesql` call, two of them the statement and its declarations).
pub(crate) fn max_params(dialect: Dialect) -> usize {
    match dialect {
        Dialect::Postgres | Dialect::MySql => 65_535,
        Dialect::Sqlite => 32_766,
        Dialect::SqlServer => 2_098,
        _ => 999,
    }
}

/// Most rows one `VALUES` list may hold (SQL Server's row constructor limit).
fn max_rows(dialect: Dialect) -> usize {
    if dialect == Dialect::SqlServer {
        1_000
    } else {
        usize::MAX
    }
}

fn invalid_name(name: &str) -> ImportError {
    ImportError::Invalid(format!("{name:?} cannot be used as a name"))
}

fn quote(dialect: Dialect, name: &str) -> Result<String, ImportError> {
    dialect.quote_ident(name).map_err(|_| invalid_name(name))
}

fn qualified(dialect: Dialect, table: &TableName) -> Result<String, ImportError> {
    if table.table.trim().is_empty() {
        return Err(ImportError::Invalid("Enter the table name".to_owned()));
    }
    dialect
        .qualified(&table.schema, &table.table)
        .map_err(|_| invalid_name(&table.table))
}

/// `CREATE TABLE` for a new table with optional primary key columns.
///
/// # Errors
/// [`ImportError::Invalid`] for empty or repeated names, invalid types or a
/// key column that is not a column.
pub fn create_table_sql(
    dialect: Dialect,
    table: &TableName,
    columns: &[NewColumn],
    primary_key: &[String],
) -> Result<String, ImportError> {
    if columns.is_empty() {
        return Err(ImportError::Invalid(
            "A new table needs at least one column".to_owned(),
        ));
    }
    let mut lines = Vec::with_capacity(columns.len() + 1);
    for (index, column) in columns.iter().enumerate() {
        if column.name.trim().is_empty() {
            return Err(ImportError::Invalid("Every column needs a name".to_owned()));
        }
        if columns[..index]
            .iter()
            .any(|other| other.name == column.name)
        {
            return Err(ImportError::Invalid(format!(
                "Column {:?} appears twice",
                column.name
            )));
        }
        let data_type = column.data_type.trim();
        if !valid_type(dialect, data_type) {
            return Err(ImportError::Invalid(format!(
                "{data_type:?} is not a valid type for column {:?}",
                column.name
            )));
        }
        lines.push(format!("  {} {data_type}", quote(dialect, &column.name)?));
    }
    if !primary_key.is_empty() {
        if let Some(missing) = primary_key
            .iter()
            .find(|key| !columns.iter().any(|c| &c.name == *key))
        {
            return Err(ImportError::Invalid(format!(
                "Key column {missing:?} is not a column"
            )));
        }
        lines.push(format!(
            "  PRIMARY KEY ({})",
            quoted_list(dialect, primary_key)?
        ));
    }
    Ok(format!(
        "CREATE TABLE {} (\n{}\n)",
        qualified(dialect, table)?,
        lines.join(",\n")
    ))
}

fn quoted_list(dialect: Dialect, names: &[String]) -> Result<String, ImportError> {
    Ok(names
        .iter()
        .map(|name| quote(dialect, name))
        .collect::<Result<Vec<_>, _>>()?
        .join(", "))
}

/// The statements of one import, quoted once.
#[derive(Clone, Debug)]
pub(crate) struct Statements {
    dialect: Dialect,
    table: String,
    columns: Vec<String>,
    key: Vec<String>,
    conflict: String,
    policy: ConflictPolicy,
}

impl Statements {
    /// # Errors
    /// [`ImportError::Invalid`] for unusable names, or skip/update without a key.
    pub(crate) fn new(
        dialect: Dialect,
        table: &TableName,
        (columns, key): (&[String], &[String]),
        policy: ConflictPolicy,
    ) -> Result<Self, ImportError> {
        if policy != ConflictPolicy::Error && key.is_empty() {
            return Err(ImportError::Invalid(
                "Choose the key columns that identify existing rows".to_owned(),
            ));
        }
        let quoted = |names: &[String]| {
            names
                .iter()
                .map(|name| quote(dialect, name))
                .collect::<Result<Vec<_>, _>>()
        };
        let mut statements = Self {
            dialect,
            table: qualified(dialect, table)?,
            columns: quoted(columns)?,
            key: quoted(key)?,
            conflict: String::new(),
            policy,
        };
        statements.conflict = statements.conflict_clause(policy);
        Ok(statements)
    }

    fn conflict_clause(&self, policy: ConflictPolicy) -> String {
        let others: Vec<&String> = self
            .columns
            .iter()
            .filter(|c| !self.key.contains(c))
            .collect();
        let first_key = self.key.first().cloned().unwrap_or_default();
        match (policy, self.dialect) {
            // SQL Server has no conflict clause: skip and update are a MERGE.
            (ConflictPolicy::Error, _) | (_, Dialect::SqlServer) => String::new(),
            (ConflictPolicy::Update, Dialect::MySql) if !others.is_empty() => {
                let sets: Vec<String> = others
                    .iter()
                    .map(|c| format!("{c} = VALUES({c})"))
                    .collect();
                format!(" ON DUPLICATE KEY UPDATE {}", sets.join(", "))
            }
            // A no-op update: the duplicate is kept as it is.
            (_, Dialect::MySql) => format!(" ON DUPLICATE KEY UPDATE {first_key} = {first_key}"),
            (ConflictPolicy::Update, _) if !others.is_empty() => {
                let sets: Vec<String> = others
                    .iter()
                    .map(|c| format!("{c} = excluded.{c}"))
                    .collect();
                format!(
                    " ON CONFLICT ({}) DO UPDATE SET {}",
                    self.key.join(", "),
                    sets.join(", ")
                )
            }
            _ => format!(" ON CONFLICT ({}) DO NOTHING", self.key.join(", ")),
        }
    }

    fn placeholder(&self, number: usize) -> String {
        match self.dialect {
            Dialect::MySql => "?".to_owned(),
            Dialect::Sqlite => format!("?{number}"),
            Dialect::SqlServer => format!("@P{number}"),
            _ => format!("${number}"),
        }
    }

    /// Placeholder tuples `(…), (…)` for `rows` rows of `width` values.
    fn tuples(&self, rows: usize, width: usize) -> Vec<String> {
        (0..rows)
            .map(|row| {
                let values: Vec<String> = (1..=width)
                    .map(|column| self.placeholder(row * width + column))
                    .collect();
                format!("({})", values.join(", "))
            })
            .collect()
    }

    /// Rows per `INSERT` within the dialect's parameter limit.
    pub(crate) fn rows_per_insert(&self, cap: usize) -> usize {
        (max_params(self.dialect) / self.columns.len().max(1))
            .min(max_rows(self.dialect))
            .clamp(1, cap.max(1))
    }

    /// `INSERT INTO t (…) VALUES (…), … <conflict clause>` for `rows` rows.
    pub(crate) fn insert(&self, rows: usize) -> String {
        let tuples = self.tuples(rows, self.columns.len());
        if self.dialect == Dialect::SqlServer && self.policy != ConflictPolicy::Error {
            let names = (&self.columns[..], &self.key[..]);
            return crate::merge::merge(&self.table, names, &tuples, self.policy);
        }
        format!(
            "INSERT INTO {} ({}) VALUES {}{}",
            self.table,
            self.columns.join(", "),
            tuples.join(", "),
            self.conflict
        )
    }

    /// A query returning one row per existing key among `keys` key tuples.
    pub(crate) fn existing_keys(&self, keys: usize) -> String {
        let width = self.key.len();
        let condition = match (width, self.dialect) {
            (1, _) => {
                let values: Vec<String> = (1..=keys).map(|n| self.placeholder(n)).collect();
                format!("{} IN ({})", self.key.join(""), values.join(", "))
            }
            // SQLite compares row values only with VALUES lists, which lose
            // the column affinity; T-SQL has no row-value comparisons.
            (_, Dialect::Sqlite | Dialect::SqlServer) => (0..keys)
                .map(|row| {
                    let terms: Vec<String> = self
                        .key
                        .iter()
                        .enumerate()
                        .map(|(i, key)| {
                            format!("{key} = {}", self.placeholder(row * width + i + 1))
                        })
                        .collect();
                    format!("({})", terms.join(" AND "))
                })
                .collect::<Vec<_>>()
                .join(" OR "),
            _ => format!(
                "({}) IN ({})",
                self.key.join(", "),
                self.tuples(keys, width).join(", ")
            ),
        };
        format!("SELECT 1 FROM {} WHERE {condition}", self.table)
    }

    /// Empty the table: `TRUNCATE` where it is transactional, else `DELETE`.
    pub(crate) fn truncate(&self) -> String {
        match self.dialect {
            Dialect::Postgres => format!("TRUNCATE TABLE {}", self.table),
            _ => format!("DELETE FROM {}", self.table),
        }
    }

    pub(crate) fn drop_table(&self) -> String {
        format!("DROP TABLE {}", self.table)
    }

    pub(crate) fn begin(&self) -> &'static str {
        match self.dialect {
            Dialect::MySql => "START TRANSACTION",
            Dialect::SqlServer => "BEGIN TRANSACTION",
            _ => "BEGIN",
        }
    }

    /// True when `CREATE TABLE` commits implicitly (`MySQL`), so it runs
    /// before the transaction and a failed import drops the table again.
    pub(crate) fn ddl_commits(&self) -> bool {
        self.dialect == Dialect::MySql
    }
}

#[cfg(test)]
#[path = "tests/sql.rs"]
mod tests;
