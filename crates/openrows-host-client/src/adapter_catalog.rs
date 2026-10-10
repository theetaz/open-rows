// SPDX-License-Identifier: Apache-2.0
//! Catalog facet forwards native reads; pure diagram assembly uses these same calls.
use crate::HostedConnection;
use openrows_driver_api::catalog::{
    ColumnSummary, DatabaseSummary, ObjectKind, ObjectSummary, SchemaSummary, TableStructure,
};
use openrows_driver_api::{BoxFuture, Catalog, DriverError};
use openrows_driver_api::{
    inspection::{ObjectDetail, ObjectIdentity},
    schema::TableDetail,
};
use openrows_host_protocol::{CatalogRequest, Control, FacetReply, Reply};
impl HostedConnection {
    pub(crate) async fn facet(&mut self, control: Control) -> Result<FacetReply, DriverError> {
        match self.call(&control).await? {
            Reply::Facet {
                value,
                in_transaction,
            } => {
                self.transaction = in_transaction;
                Ok(value)
            }
            _ => Err(self.protocol()),
        }
    }
}
impl Catalog for HostedConnection {
    fn list_databases(&mut self) -> BoxFuture<'_, Result<Vec<DatabaseSummary>, DriverError>> {
        Box::pin(async move {
            match self
                .facet(Control::CatalogRead(CatalogRequest::Databases))
                .await?
            {
                FacetReply::Databases(values) => Ok(values.into_iter().map(Into::into).collect()),
                _ => Err(self.protocol()),
            }
        })
    }
    fn list_schemas(&mut self) -> BoxFuture<'_, Result<Vec<SchemaSummary>, DriverError>> {
        Box::pin(async move {
            match self
                .facet(Control::CatalogRead(CatalogRequest::Schemas))
                .await?
            {
                FacetReply::Schemas(values) => Ok(values.into_iter().map(Into::into).collect()),
                _ => Err(self.protocol()),
            }
        })
    }
    fn list_objects<'a>(
        &'a mut self,
        schema: &'a str,
        kinds: &'a [ObjectKind],
    ) -> BoxFuture<'a, Result<Vec<ObjectSummary>, DriverError>> {
        Box::pin(async move {
            let request = CatalogRequest::Objects {
                schema: schema.into(),
                kinds: kinds.iter().copied().map(Into::into).collect(),
            };
            match self.facet(Control::CatalogRead(request)).await? {
                FacetReply::Objects(values) => Ok(values.into_iter().map(Into::into).collect()),
                _ => Err(self.protocol()),
            }
        })
    }
    fn search_index(&mut self) -> BoxFuture<'_, Result<Vec<ObjectSummary>, DriverError>> {
        Box::pin(async move {
            match self
                .facet(Control::CatalogRead(CatalogRequest::SearchIndex))
                .await?
            {
                FacetReply::Objects(values) => Ok(values.into_iter().map(Into::into).collect()),
                _ => Err(self.protocol()),
            }
        })
    }
    fn table_columns<'a>(
        &'a mut self,
        schema: &'a str,
        table: &'a str,
    ) -> BoxFuture<'a, Result<Vec<ColumnSummary>, DriverError>> {
        Box::pin(async move {
            let request = CatalogRequest::Columns {
                schema: schema.into(),
                table: table.into(),
            };
            match self.facet(Control::CatalogRead(request)).await? {
                FacetReply::Columns(values) => Ok(values.into_iter().map(Into::into).collect()),
                _ => Err(self.protocol()),
            }
        })
    }
    fn table_structure<'a>(
        &'a mut self,
        schema: &'a str,
        table: &'a str,
    ) -> BoxFuture<'a, Result<TableStructure, DriverError>> {
        Box::pin(async move {
            let request = CatalogRequest::Structure {
                schema: schema.into(),
                table: table.into(),
            };
            match self.facet(Control::CatalogRead(request)).await? {
                FacetReply::Structure(value) => Ok(value.into()),
                _ => Err(self.protocol()),
            }
        })
    }
    fn table_detail<'a>(
        &'a mut self,
        schema: &'a str,
        table: &'a str,
    ) -> BoxFuture<'a, Result<TableDetail, DriverError>> {
        Box::pin(async move {
            let request = CatalogRequest::Detail {
                schema: schema.into(),
                table: table.into(),
            };
            match self.facet(Control::CatalogRead(request)).await? {
                FacetReply::Detail(value) => Ok(value.into()),
                _ => Err(self.protocol()),
            }
        })
    }
    fn schema_fingerprint(&mut self) -> BoxFuture<'_, Result<Option<String>, DriverError>> {
        Box::pin(async move {
            match self
                .facet(Control::CatalogRead(CatalogRequest::Fingerprint))
                .await?
            {
                FacetReply::Fingerprint(value) => Ok(value),
                _ => Err(self.protocol()),
            }
        })
    }
    fn object_detail<'a>(
        &'a mut self,
        object: &'a ObjectIdentity,
    ) -> BoxFuture<'a, Result<ObjectDetail, DriverError>> {
        Box::pin(async move {
            let request = CatalogRequest::ObjectDetail {
                schema: object.schema.clone(),
                name: object.name.clone(),
                kind: object.kind.into(),
                arguments: object.arguments.clone(),
            };
            match self.facet(Control::CatalogRead(request)).await? {
                FacetReply::ObjectDetail(value) => Ok(value.into()),
                _ => Err(self.protocol()),
            }
        })
    }
}
