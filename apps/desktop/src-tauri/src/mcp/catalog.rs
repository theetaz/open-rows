// SPDX-License-Identifier: Apache-2.0
//! Bounded catalog definitions; database comments never enter tool instructions.
use openrows_ai::sharing::SchemaObject;
use openrows_driver_api::{Catalog, DriverError, catalog::ObjectKind};
pub(super) async fn collect(
    catalog: &mut dyn Catalog,
    tool: String,
    (schema, object): (String, String),
) -> Result<Vec<SchemaObject>, DriverError> {
    match tool.as_str() {
        "list_schemas" => Ok(catalog
            .list_schemas()
            .await?
            .into_iter()
            .take(128)
            .map(|item| SchemaObject {
                name: item.name,
                ddl: String::new(),
            })
            .collect()),
        "list_objects" => Ok(catalog
            .list_objects(&schema, &ObjectKind::ALL)
            .await?
            .into_iter()
            .take(128)
            .map(|item| SchemaObject {
                name: format!("{}.{}", item.schema, item.name),
                ddl: item.kind.as_str().into(),
            })
            .collect()),
        "describe_object" => {
            let columns = catalog.table_columns(&schema, &object).await?;
            let ddl = columns
                .iter()
                .take(200)
                .map(|column| format!("{} {}", column.name, column.data_type))
                .collect::<Vec<_>>()
                .join(", ");
            Ok(vec![SchemaObject {
                name: format!("{schema}.{object}"),
                ddl,
            }])
        }
        _ => Ok(Vec::new()),
    }
}
