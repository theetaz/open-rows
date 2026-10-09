// SPDX-License-Identifier: Apache-2.0
//! SQL Server has no `ON CONFLICT`: an import that skips or updates rows
//! whose key exists is one `MERGE` per batch of rows, matching on the key.
//! A batch that holds the same key twice fails in the server (`MERGE`
//! may not change one row twice).
use crate::sql::ConflictPolicy;

/// `MERGE INTO t AS d USING (VALUES …) AS s (cols) ON d.k = s.k [WHEN
/// MATCHED THEN UPDATE SET …] WHEN NOT MATCHED THEN INSERT …;` for
/// `tuples` of placeholders; names are quoted.
pub(crate) fn merge(
    table: &str,
    (columns, key): (&[String], &[String]),
    tuples: &[String],
    policy: ConflictPolicy,
) -> String {
    let on: Vec<String> = key.iter().map(|k| format!("d.{k} = s.{k}")).collect();
    let others: Vec<String> = columns
        .iter()
        .filter(|column| !key.contains(column))
        .map(|column| format!("{column} = s.{column}"))
        .collect();
    let update = if policy == ConflictPolicy::Update && !others.is_empty() {
        format!(" WHEN MATCHED THEN UPDATE SET {}", others.join(", "))
    } else {
        String::new()
    };
    let sources: Vec<String> = columns.iter().map(|column| format!("s.{column}")).collect();
    format!(
        "MERGE INTO {table} AS d USING (VALUES {}) AS s ({}) ON {}{update} \
         WHEN NOT MATCHED THEN INSERT ({}) VALUES ({});",
        tuples.join(", "),
        columns.join(", "),
        on.join(" AND "),
        columns.join(", "),
        sources.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_skip_or_update_existing_keys() {
        let columns = ["[id]".to_owned(), "[name]".to_owned()];
        let key = ["[id]".to_owned()];
        let tuples = ["(@P1, @P2)".to_owned(), "(@P3, @P4)".to_owned()];
        let update = merge(
            "[dbo].[t]",
            (&columns, &key),
            &tuples,
            ConflictPolicy::Update,
        );
        assert_eq!(
            update,
            "MERGE INTO [dbo].[t] AS d USING (VALUES (@P1, @P2), (@P3, @P4)) AS s ([id], [name]) \
             ON d.[id] = s.[id] WHEN MATCHED THEN UPDATE SET [name] = s.[name] \
             WHEN NOT MATCHED THEN INSERT ([id], [name]) VALUES (s.[id], s.[name]);"
        );
        let skip = merge("[dbo].[t]", (&columns, &key), &tuples, ConflictPolicy::Skip);
        assert!(!skip.contains("WHEN MATCHED"));
    }
}
