// SPDX-License-Identifier: Apache-2.0
//! Native facet calls stay on the dedicated connection worker.
use openrows_driver_api::{Connection, DriverError};
use openrows_host_protocol::{CatalogRequest, EditorRequest, FacetReply, Reply, WireRowEditInfo};

pub(crate) async fn catalog(
    connection: &mut dyn Connection,
    request: CatalogRequest,
) -> Result<Reply, DriverError> {
    let catalog = openrows_driver_api::connection::catalog(connection)?;
    let value = match request {
        CatalogRequest::Databases => {
            FacetReply::Databases(bounded(catalog.list_databases().await?)?)
        }
        CatalogRequest::Schemas => FacetReply::Schemas(bounded(catalog.list_schemas().await?)?),
        CatalogRequest::Objects { schema, kinds } => {
            let kinds: Vec<_> = kinds.into_iter().map(Into::into).collect();
            FacetReply::Objects(bounded(catalog.list_objects(&schema, &kinds).await?)?)
        }
        CatalogRequest::SearchIndex => FacetReply::Objects(bounded(catalog.search_index().await?)?),
        CatalogRequest::Columns { schema, table } => {
            FacetReply::Columns(bounded(catalog.table_columns(&schema, &table).await?)?)
        }
        CatalogRequest::Structure { schema, table } => {
            FacetReply::Structure(catalog.table_structure(&schema, &table).await?.into())
        }
        CatalogRequest::Detail { schema, table } => {
            FacetReply::Detail(catalog.table_detail(&schema, &table).await?.into())
        }
        CatalogRequest::Fingerprint => FacetReply::Fingerprint(catalog.schema_fingerprint().await?),
        CatalogRequest::ObjectDetail {
            schema,
            name,
            kind,
            arguments,
        } => {
            let object = openrows_driver_api::inspection::ObjectIdentity {
                schema,
                name,
                kind: kind.into(),
                arguments,
            };
            FacetReply::ObjectDetail(catalog.object_detail(&object).await?.into())
        }
    };
    Ok(Reply::Facet {
        value,
        in_transaction: connection.in_transaction(),
    })
}
pub(crate) async fn edit(
    connection: &mut dyn Connection,
    request: EditorRequest,
) -> Result<Reply, DriverError> {
    let editor = openrows_driver_api::connection::editor(connection)?;
    let value = match request {
        EditorRequest::Info { schema, table } => {
            let info = editor.row_edit_info(&schema, &table).await?;
            FacetReply::EditInfo(WireRowEditInfo {
                key: info.key,
                protected: info.protected,
                row_id: info.row_id,
            })
        }
        EditorRequest::Snapshot {
            schema,
            table,
            keys,
        } => FacetReply::Snapshot(editor.edit_snapshot((&schema, &table), &keys).await?),
        EditorRequest::Commit(edits) => {
            let edits: Vec<_> = edits
                .into_iter()
                .map(|edit| openrows_sql::select::BoundSql {
                    sql: edit.sql,
                    params: edit.params,
                })
                .collect();
            editor.commit_edits(&edits).await?;
            FacetReply::Committed
        }
        EditorRequest::Ddl(statements) => {
            #[cfg(feature = "test-hooks")]
            if statements == ["OPENROWS_TEST_COMMIT_ABORT"] {
                editor
                    .commit_ddl(&["CREATE TABLE committed_once(id integer)".into()])
                    .await?;
                std::process::abort();
            }
            editor.commit_ddl(&statements).await?;
            FacetReply::Committed
        }
    };
    Ok(Reply::Facet {
        value,
        in_transaction: connection.in_transaction(),
    })
}
fn bounded<T, U: From<T>>(values: Vec<T>) -> Result<Vec<U>, DriverError> {
    if values.len() > 65536 {
        return Err(DriverError::Unsupported(
            "host catalog response exceeds 65536 entries".into(),
        ));
    }
    Ok(values.into_iter().map(Into::into).collect())
}
