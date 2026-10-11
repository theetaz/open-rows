// SPDX-License-Identifier: Apache-2.0
//! Prove base-table provenance and exclude `PostgreSQL`'s implicit user-code paths.
use crate::{McpError, rows::Collector};
use openrows_driver_api::{Connection, catalog::ObjectKind};
pub(crate) async fn validate(
    connection: &mut dyn Connection,
    source: Option<&(String, String)>,
    engine: crate::ReadEngine,
) -> Result<(), McpError> {
    let Some((schema, table)) = source else {
        return Ok(());
    };
    let catalog = openrows_driver_api::catalog(connection).map_err(|_| McpError::Unsafe)?;
    let objects = catalog
        .list_objects(schema, &[ObjectKind::Table])
        .await
        .map_err(|_| McpError::Database)?;
    if !objects
        .iter()
        .any(|object| &object.name == table && object.kind == ObjectKind::Table)
    {
        return Err(McpError::Unsafe);
    }
    match engine {
        crate::ReadEngine::Postgres => postgres_table(connection, (schema, table)).await?,
        crate::ReadEngine::Mysql | crate::ReadEngine::Mariadb => {
            mysql_table(connection, (schema, table)).await?;
        }
        crate::ReadEngine::Sqlite | crate::ReadEngine::LibsqlFile => {}
        crate::ReadEngine::Libsql
        | crate::ReadEngine::Sqlserver
        | crate::ReadEngine::Clickhouse
        | crate::ReadEngine::Duckdb => return Err(McpError::Unsafe),
    }
    Ok(())
}
async fn postgres_table(
    connection: &mut dyn Connection,
    (schema, table): (&str, &str),
) -> Result<(), McpError> {
    let name = openrows_sql::Dialect::Postgres
        .qualified(schema, table)
        .map_err(|_| McpError::Unsafe)?;
    connection
        .execute(
            &format!("LOCK TABLE {name} IN ACCESS SHARE MODE"),
            &mut openrows_driver_api::DiscardSink,
        )
        .await
        .map_err(|_| McpError::Database)?;
    let qualified = name.replace('\'', "''");
    // RLS, inheritance, generated expressions and custom types/operators can invoke
    // user functions implicitly, even when the SELECT contains no function call.
    let sql = format!(
        "SELECT c.relkind = 'r' AND NOT c.relrowsecurity AND NOT c.relhassubclass
        AND NOT EXISTS (SELECT 1 FROM pg_catalog.pg_inherits i WHERE i.inhrelid=c.oid)
        AND NOT EXISTS (SELECT 1 FROM pg_catalog.pg_attribute a
            JOIN pg_catalog.pg_type t ON t.oid=a.atttypid
            JOIN pg_catalog.pg_namespace n ON n.oid=t.typnamespace
            LEFT JOIN pg_catalog.pg_collation co ON co.oid=a.attcollation
            LEFT JOIN pg_catalog.pg_namespace cn ON cn.oid=co.collnamespace
            WHERE a.attrelid=c.oid AND a.attnum>0 AND NOT a.attisdropped
            AND (n.nspname <> 'pg_catalog' OR t.typtype <> 'b' OR t.typelem <> 0
                OR a.attgenerated <> '' OR (a.attcollation <> 0 AND cn.nspname <> 'pg_catalog')))
        FROM pg_catalog.pg_class c WHERE c.oid=pg_catalog.to_regclass('{qualified}')"
    );
    let mut result = Collector::new(false);
    connection
        .execute(&sql, &mut result)
        .await
        .map_err(|_| McpError::Database)?;
    if result
        .rows
        .first()
        .and_then(|row| row.first())
        .and_then(Option::as_deref)
        != Some("true")
    {
        return Err(McpError::Unsafe);
    }
    Ok(())
}

async fn mysql_table(
    connection: &mut dyn Connection,
    (schema, table): (&str, &str),
) -> Result<(), McpError> {
    let name = openrows_sql::Dialect::MySql
        .qualified(schema, table)
        .map_err(|_| McpError::Unsafe)?;
    // Acquire a transaction-held metadata lock without evaluating any table values.
    connection
        .execute(
            &format!("SELECT 0 FROM {name} WHERE 1=0"),
            &mut openrows_driver_api::DiscardSink,
        )
        .await
        .map_err(|_| McpError::Database)?;
    let schema = mysql_literal(schema);
    let table = mysql_literal(table);
    // MariaDB permits UDFs in virtual generated expressions. Refuse all generated
    // columns, rather than trusting read-only mode to constrain implicit user code.
    let sql = format!(
        "SELECT COUNT(*) FROM information_schema.tables t WHERE t.TABLE_SCHEMA={schema} AND t.TABLE_NAME={table} AND t.TABLE_TYPE='BASE TABLE' AND NOT EXISTS (SELECT 1 FROM information_schema.columns c WHERE c.TABLE_SCHEMA={schema} AND c.TABLE_NAME={table} AND (c.EXTRA LIKE '%GENERATED%' OR c.GENERATION_EXPRESSION <> ''))"
    );
    let mut result = Collector::new(false);
    connection
        .execute(&sql, &mut result)
        .await
        .map_err(|_| McpError::Database)?;
    if result
        .rows
        .first()
        .and_then(|row| row.first())
        .and_then(Option::as_deref)
        != Some("1")
    {
        return Err(McpError::Unsafe);
    }
    Ok(())
}
fn mysql_literal(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(value.len() * 2 + 28);
    output.push_str("CONVERT(0x");
    for byte in value.bytes() {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 15)]));
    }
    output.push_str(" USING utf8mb4)");
    output
}
