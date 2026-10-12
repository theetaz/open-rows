// SPDX-License-Identifier: Apache-2.0
//! Selected-table metadata is validated before any detail reads.
use super::dto::DiagramTarget;
use crate::connections::catalog::read_catalog;
use crate::connections::{Connections, IpcError};
use openrows_driver_api::diagram::{SchemaDiagram, read_selected};

/// Run selected-table reads on the database metadata session.
pub(super) async fn read(
    state: &Connections,
    target: &DiagramTarget,
    tables: Vec<String>,
) -> Result<SchemaDiagram, IpcError> {
    let schema = target.schema.clone();
    read_catalog(
        state,
        (&target.profile_id, &target.database),
        move |catalog| Box::pin(async move { read_selected(catalog, &schema, &tables).await }),
    )
    .await
}

/// Validate the public selection before taking a metadata session.
pub(super) fn validate(tables: &[String]) -> Result<(), IpcError> {
    if tables.is_empty() || tables.len() > super::MAX_TABLES {
        return Err(IpcError::invalid("Select between 1 and 400 tables"));
    }
    if tables.iter().any(String::is_empty) {
        return Err(IpcError::invalid("Selected table names cannot be empty"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_is_explicit_and_bounded() {
        assert!(validate(&[]).is_err());
        assert!(validate(&[String::new()]).is_err());
        assert!(validate(&vec!["table".into(); 401]).is_err());
        assert!(validate(&vec!["table".into(); 400]).is_ok());
        assert!(validate(&["quoted.table'".into()]).is_ok());
    }
}
