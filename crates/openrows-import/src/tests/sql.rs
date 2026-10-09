// SPDX-License-Identifier: Apache-2.0
use super::*;

fn users() -> TableName {
    TableName {
        schema: "public".to_owned(),
        table: "users".to_owned(),
    }
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|&name| name.to_owned()).collect()
}

fn statements(
    dialect: Dialect,
    policy: ConflictPolicy,
    key: &[&str],
) -> Result<Statements, ImportError> {
    Statements::new(
        dialect,
        &users(),
        (&names(&["id", "name"]), &names(key)),
        policy,
    )
}

#[test]
fn inserts_are_multi_row_with_dialect_placeholders() -> Result<(), ImportError> {
    let pg = statements(Dialect::Postgres, ConflictPolicy::Error, &[])?;
    assert_eq!(
        pg.insert(2),
        "INSERT INTO \"public\".\"users\" (\"id\", \"name\") VALUES ($1, $2), ($3, $4)"
    );
    let mysql = statements(Dialect::MySql, ConflictPolicy::Error, &[])?;
    assert_eq!(
        mysql.insert(2),
        "INSERT INTO `public`.`users` (`id`, `name`) VALUES (?, ?), (?, ?)"
    );
    let sqlite = statements(Dialect::Sqlite, ConflictPolicy::Error, &[])?;
    assert!(sqlite.insert(1).ends_with("VALUES (?1, ?2)"));
    assert_eq!(pg.rows_per_insert(1_000), 1_000);
    assert_eq!(pg.rows_per_insert(100_000), 32_767);
    Ok(())
}

#[test]
fn conflict_clauses_follow_policy_and_dialect() -> Result<(), ImportError> {
    let skip = statements(Dialect::Postgres, ConflictPolicy::Skip, &["id"])?;
    assert!(skip.insert(1).ends_with(" ON CONFLICT (\"id\") DO NOTHING"));
    let upsert = statements(Dialect::Sqlite, ConflictPolicy::Update, &["id"])?;
    assert!(
        upsert
            .insert(1)
            .ends_with(" ON CONFLICT (\"id\") DO UPDATE SET \"name\" = excluded.\"name\"")
    );
    let mysql = statements(Dialect::MySql, ConflictPolicy::Update, &["id"])?;
    assert!(
        mysql
            .insert(1)
            .ends_with(" ON DUPLICATE KEY UPDATE `name` = VALUES(`name`)")
    );
    let mysql_skip = statements(Dialect::MySql, ConflictPolicy::Skip, &["id"])?;
    assert!(
        mysql_skip
            .insert(1)
            .ends_with(" ON DUPLICATE KEY UPDATE `id` = `id`")
    );
    let key_only = Statements::new(
        Dialect::Postgres,
        &users(),
        (&names(&["id"]), &names(&["id"])),
        ConflictPolicy::Update,
    )?;
    assert!(key_only.insert(1).ends_with("DO NOTHING"));
    assert!(statements(Dialect::Postgres, ConflictPolicy::Skip, &[]).is_err());
    Ok(())
}

#[test]
fn existing_key_probes_bind_every_key() -> Result<(), ImportError> {
    let single = statements(Dialect::Postgres, ConflictPolicy::Error, &["id"])?;
    assert_eq!(
        single.existing_keys(3),
        "SELECT 1 FROM \"public\".\"users\" WHERE \"id\" IN ($1, $2, $3)"
    );
    let pair = statements(Dialect::MySql, ConflictPolicy::Error, &["id", "name"])?;
    assert_eq!(
        pair.existing_keys(2),
        "SELECT 1 FROM `public`.`users` WHERE (`id`, `name`) IN ((?, ?), (?, ?))"
    );
    let sqlite = statements(Dialect::Sqlite, ConflictPolicy::Error, &["id", "name"])?;
    assert_eq!(
        sqlite.existing_keys(2),
        "SELECT 1 FROM \"public\".\"users\" WHERE (\"id\" = ?1 AND \"name\" = ?2) OR (\"id\" = ?3 AND \"name\" = ?4)"
    );
    Ok(())
}

#[test]
fn truncation_and_transactions_suit_the_engine() -> Result<(), ImportError> {
    let pg = statements(Dialect::Postgres, ConflictPolicy::Error, &[])?;
    assert_eq!(pg.truncate(), "TRUNCATE TABLE \"public\".\"users\"");
    assert_eq!(pg.begin(), "BEGIN");
    assert!(!pg.ddl_commits());
    let mysql = statements(Dialect::MySql, ConflictPolicy::Error, &[])?;
    assert_eq!(mysql.truncate(), "DELETE FROM `public`.`users`");
    assert_eq!(mysql.begin(), "START TRANSACTION");
    assert!(mysql.ddl_commits());
    assert_eq!(mysql.drop_table(), "DROP TABLE `public`.`users`");
    Ok(())
}

#[test]
fn new_tables_validate_names_types_and_keys() -> Result<(), ImportError> {
    let column = |name: &str, data_type: &str| NewColumn {
        name: name.to_owned(),
        data_type: data_type.to_owned(),
    };
    let sql = create_table_sql(
        Dialect::Postgres,
        &users(),
        &[column("id", "bigint"), column("na\"me", "numeric(10, 2)")],
        &names(&["id"]),
    )?;
    assert_eq!(
        sql,
        "CREATE TABLE \"public\".\"users\" (\n  \"id\" bigint,\n  \"na\"\"me\" numeric(10, 2),\n  PRIMARY KEY (\"id\")\n)"
    );
    let refused = [
        create_table_sql(Dialect::Postgres, &users(), &[], &[]),
        create_table_sql(
            Dialect::Postgres,
            &users(),
            &[column("a", "int); DROP TABLE x; --")],
            &[],
        ),
        create_table_sql(
            Dialect::Postgres,
            &users(),
            &[column("a", "int"), column("a", "int")],
            &[],
        ),
        create_table_sql(Dialect::Postgres, &users(), &[column(" ", "int")], &[]),
        create_table_sql(
            Dialect::Postgres,
            &users(),
            &[column("a", "int")],
            &names(&["b"]),
        ),
        create_table_sql(
            Dialect::Postgres,
            &TableName {
                schema: "public".to_owned(),
                table: String::new(),
            },
            &[column("a", "int")],
            &[],
        ),
    ];
    assert!(refused.iter().all(Result::is_err));
    Ok(())
}

#[test]
fn sql_server_imports_bind_at_p_and_merge_on_conflicts() -> Result<(), ImportError> {
    let plain = statements(Dialect::SqlServer, ConflictPolicy::Error, &[])?;
    assert_eq!(
        plain.insert(2),
        "INSERT INTO [public].[users] ([id], [name]) VALUES (@P1, @P2), (@P3, @P4)"
    );
    // 2 098 parameters over two columns, but at most 1 000 rows per VALUES list.
    assert_eq!(plain.rows_per_insert(usize::MAX), 1_000);
    assert_eq!(plain.begin(), "BEGIN TRANSACTION");
    assert!(!plain.ddl_commits());
    let update = statements(Dialect::SqlServer, ConflictPolicy::Update, &["id"])?;
    let merge = update.insert(1);
    assert!(merge.starts_with("MERGE INTO [public].[users] AS d USING (VALUES (@P1, @P2))"));
    assert!(merge.contains("WHEN MATCHED THEN UPDATE SET [name] = s.[name]"));
    assert!(!merge.contains("ON CONFLICT"));
    let composite = Statements::new(
        Dialect::SqlServer,
        &users(),
        (&names(&["id", "name"]), &names(&["id", "name"])),
        ConflictPolicy::Skip,
    )?;
    assert_eq!(
        composite.existing_keys(2),
        "SELECT 1 FROM [public].[users] WHERE ([id] = @P1 AND [name] = @P2) OR \
         ([id] = @P3 AND [name] = @P4)"
    );
    let created = create_table_sql(
        Dialect::SqlServer,
        &users(),
        &[NewColumn {
            name: "id".to_owned(),
            data_type: "nvarchar(max)".to_owned(),
        }],
        &[],
    )?;
    assert_eq!(
        created,
        "CREATE TABLE [public].[users] (\n  [id] nvarchar(max)\n)"
    );
    Ok(())
}
