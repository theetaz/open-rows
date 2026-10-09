// SPDX-License-Identifier: Apache-2.0
//! A staged structure change set as one `ALTER TABLE` statement (`MySQL`
//! 8 applies it atomically). Changed columns are rewritten from the
//! server's own `SHOW CREATE TABLE` definition, so character set,
//! collation, `AUTO_INCREMENT`, `ON UPDATE` and generation clauses survive a
//! `MODIFY`; a pure rename is `RENAME COLUMN`.
use openrows_driver_api::schema::{DefaultValue, IndexMethod, NewColumn, NewIndex, SchemaChange};
use openrows_driver_api::structure::{
    AlterError, AlterPlan, AlterTable, ColumnEdits, Comment, IndexDrops, StagedChanges,
};
use openrows_sql::Dialect;
use openrows_sql::ddl::{ColumnDefinition, TableDefinition, string_literal, valid_type};

const DIALECT: Dialect = Dialect::MySql;

/// Validate `changes` against the catalog and generate the statement.
///
/// # Errors
/// See [`AlterError`].
pub(crate) fn alter_sql(
    target: &AlterTable<'_>,
    changes: &[SchemaChange],
) -> Result<AlterPlan, AlterError> {
    let staged = StagedChanges::stage(target, changes, IndexDrops::NotPrimary)?;
    let definition = target
        .detail
        .info
        .as_ref()
        .and_then(|info| info.definition.as_deref())
        .and_then(|sql| TableDefinition::parse(sql, DIALECT));
    let mut plan = Plan {
        table: target.table,
        definition,
        specs: Vec::new(),
        out: AlterPlan::default(),
    };
    plan.columns(target, &staged)?;
    plan.indexes(&staged)?;
    plan.table_options(&staged)?;
    if plan.specs.is_empty() {
        return Err(AlterError::Empty);
    }
    let relation = DIALECT
        .qualified(target.schema, target.table)
        .map_err(|_| AlterError::InvalidName)?;
    plan.out.statements = vec![format!(
        "ALTER TABLE {relation}\n  {}",
        plan.specs.join(",\n  ")
    )];
    plan.out
        .warnings
        .push("MySQL commits ALTER TABLE at once; it cannot be rolled back.".to_owned());
    Ok(plan.out)
}

pub(crate) fn quote(name: &str) -> Result<String, AlterError> {
    if name.is_empty() {
        return Err(AlterError::InvalidName);
    }
    DIALECT
        .quote_ident(name)
        .map_err(|_| AlterError::InvalidName)
}

fn literal(text: &str) -> Result<String, AlterError> {
    string_literal(DIALECT, text).ok_or(AlterError::InvalidName)
}

fn data_type(text: &str) -> Result<String, AlterError> {
    let trimmed = text.trim();
    if valid_type(DIALECT, trimmed) {
        Ok(trimmed.to_owned())
    } else {
        Err(AlterError::InvalidType(text.to_owned()))
    }
}

/// `None` for no default, else the SQL after `DEFAULT`; expressions are
/// parenthesised (`MySQL` 8.0.13+ expression defaults).
fn default_sql(column: &str, default: &DefaultValue) -> Result<Option<String>, AlterError> {
    match default {
        DefaultValue::None => Ok(None),
        DefaultValue::Literal(value) => literal(value).map(Some),
        DefaultValue::Expression(sql) => {
            let sql = sql.trim();
            if sql.is_empty() || sql.contains('\0') {
                Err(AlterError::InvalidDefault(column.to_owned()))
            } else {
                Ok(Some(format!("({sql})")))
            }
        }
    }
}

struct Plan<'a> {
    table: &'a str,
    definition: Option<TableDefinition>,
    specs: Vec<String>,
    out: AlterPlan,
}

impl Plan<'_> {
    fn warn(&mut self, warning: String) {
        self.out.warnings.push(warning);
    }

    fn columns(
        &mut self,
        target: &AlterTable<'_>,
        staged: &StagedChanges<'_>,
    ) -> Result<(), AlterError> {
        for &name in &staged.drops {
            self.specs.push(format!("DROP COLUMN {}", quote(name)?));
            self.warn(format!(
                "Dropping column {name:?} permanently deletes its data."
            ));
            self.out.destructive = true;
        }
        for column in &target.detail.columns {
            if let Some(edits) = staged.columns.get(column.name.as_str()) {
                self.column(&column.name, edits)?;
            }
        }
        for column in &staged.adds {
            self.add_column(column)?;
        }
        Ok(())
    }

    fn column(&mut self, name: &str, edits: &ColumnEdits<'_>) -> Result<(), AlterError> {
        let rename = edits.renamed(name);
        let redefined = edits.data_type.is_some()
            || edits.nullable.is_some()
            || edits.default.is_some()
            || edits.comment.is_some();
        if !redefined {
            if let Some(rename) = rename {
                self.specs.push(format!(
                    "RENAME COLUMN {} TO {}",
                    quote(name)?,
                    quote(rename)?
                ));
            }
            return Ok(());
        }
        let mut definition = self.current_definition(name)?;
        self.redefine(name, edits, &mut definition)?;
        let spec = match rename {
            Some(rename) => {
                definition.set_name(&quote(rename)?);
                format!("CHANGE COLUMN {} {}", quote(name)?, definition.render())
            }
            None => format!("MODIFY COLUMN {}", definition.render()),
        };
        self.specs.push(spec);
        Ok(())
    }

    /// The column's clause list from `SHOW CREATE TABLE`.
    fn current_definition(&self, name: &str) -> Result<ColumnDefinition, AlterError> {
        self.definition
            .as_ref()
            .and_then(|table| table.column(name))
            .and_then(|element| ColumnDefinition::parse(&element.text, DIALECT))
            .ok_or_else(|| AlterError::UnreadableDefinition(self.table.to_owned()))
    }

    fn redefine(
        &mut self,
        name: &str,
        edits: &ColumnEdits<'_>,
        definition: &mut ColumnDefinition,
    ) -> Result<(), AlterError> {
        if let Some(text) = edits.data_type {
            definition.set_type(&data_type(text)?);
            self.warn(format!(
                "Changing the type of {name:?} copies the table and fails or truncates values that cannot be converted."
            ));
            self.out.destructive = true;
        }
        if let Some(nullable) = edits.nullable {
            definition.set_nullable(nullable);
            if !nullable {
                self.warn(format!(
                    "NOT NULL on {name:?} fails if the column contains NULLs."
                ));
            }
        }
        if let Some(default) = edits.default {
            definition.set_default(default_sql(name, default)?.as_deref());
        }
        if let Some(Comment(comment)) = edits.comment {
            let literal = comment.map(literal).transpose()?;
            definition.set_comment(literal.as_deref());
        }
        Ok(())
    }

    fn add_column(&mut self, column: &NewColumn) -> Result<(), AlterError> {
        let mut definition =
            ColumnDefinition::new(&quote(&column.name)?, &data_type(&column.data_type)?);
        definition.set_nullable(column.nullable);
        let default = default_sql(&column.name, &column.default)?;
        if default.is_none() && !column.nullable {
            self.warn(format!(
                "Column {:?} is NOT NULL without a default; existing rows get the type's zero value.",
                column.name
            ));
        }
        definition.set_default(default.as_deref());
        let comment = column.comment.as_deref().map(literal).transpose()?;
        definition.set_comment(comment.as_deref());
        self.specs
            .push(format!("ADD COLUMN {}", definition.render()));
        Ok(())
    }

    fn indexes(&mut self, staged: &StagedChanges<'_>) -> Result<(), AlterError> {
        for &name in &staged.index_drops {
            self.specs.push(format!("DROP INDEX {}", quote(name)?));
            self.out.destructive = true;
        }
        for index in &staged.index_adds {
            let spec = add_index(index)?;
            self.specs.push(spec);
        }
        Ok(())
    }

    fn table_options(&mut self, staged: &StagedChanges<'_>) -> Result<(), AlterError> {
        if let Some(Comment(comment)) = staged.comment {
            self.specs.push(format!(
                "COMMENT = {}",
                literal(comment.unwrap_or_default())?
            ));
        }
        if let Some(name) = staged.rename {
            self.specs.push(format!("RENAME TO {}", quote(name)?));
            self.warn("Code that uses the old table name stops working.".to_owned());
        }
        Ok(())
    }
}

fn add_index(index: &NewIndex) -> Result<String, AlterError> {
    let method = match index.method {
        IndexMethod::Btree => "BTREE",
        IndexMethod::Hash => "HASH",
        other => {
            return Err(AlterError::Unsupported(format!(
                "MySQL has no {other:?} indexes; use B-tree or hash"
            )));
        }
    };
    let unique = if index.unique { "UNIQUE " } else { "" };
    let name = match index.name.as_deref().filter(|name| !name.is_empty()) {
        Some(name) => format!("{} ", quote(name)?),
        None => String::new(),
    };
    let columns = index
        .columns
        .iter()
        .map(|column| quote(column))
        .collect::<Result<Vec<_>, _>>()?
        .join(", ");
    Ok(format!(
        "ADD {unique}INDEX {name}({columns}) USING {method}"
    ))
}

#[cfg(test)]
#[path = "ddl_tests.rs"]
mod tests;
