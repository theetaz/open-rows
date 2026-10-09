// SPDX-License-Identifier: Apache-2.0
//! Product, version and flavour from `SERVERPROPERTY`: on-premises SQL
//! Server, or the Azure service the engine edition names.
use openrows_driver_api::{DriverError, ServerSummary, TlsStatus};

use crate::reply::Env;
use crate::session::SqlServerSession;

const INFO_SQL: &str = "SELECT CAST(SERVERPROPERTY('ProductVersion') AS nvarchar(128)), \
    CAST(SERVERPROPERTY('EngineEdition') AS int), DB_NAME(), SUSER_SNAME()";

/// Product name of every flavour.
pub(crate) const PRODUCT: &str = "SQL Server";

/// The service a `SERVERPROPERTY('EngineEdition')` names, if not on-premises.
pub(crate) fn flavour(engine_edition: &str) -> Option<&'static str> {
    match engine_edition {
        "5" => Some("Azure SQL Database"),
        "6" => Some("Azure Synapse Analytics"),
        "8" => Some("Azure SQL Managed Instance"),
        "9" => Some("Azure SQL Edge"),
        "11" => Some("Azure Synapse serverless SQL pool"),
        "12" => Some("Microsoft Fabric"),
        _ => None,
    }
}

/// Read the identity of the logged-in session.
pub(crate) async fn fetch(
    session: &mut SqlServerSession,
    tls: TlsStatus,
) -> Result<ServerSummary, DriverError> {
    let rows = session.text_query(INFO_SQL, &[]).await?;
    let row = rows.into_iter().next().unwrap_or_default();
    let text = |index: usize| row.get(index).cloned().flatten().unwrap_or_default();
    let database = Some(text(2))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| session.env.database.clone());
    Ok(ServerSummary {
        product: PRODUCT.to_owned(),
        version: text(0),
        flavour: flavour(&text(1)).map(str::to_owned),
        database,
        user: text(3),
        tls,
    })
}

/// A placeholder summary until [`fetch`] ran.
pub(crate) fn pending(env: &Env, tls: TlsStatus) -> ServerSummary {
    ServerSummary {
        product: PRODUCT.to_owned(),
        version: String::new(),
        flavour: None,
        database: env.database.clone(),
        user: String::new(),
        tls,
    }
}

#[cfg(test)]
mod tests {
    use super::flavour;

    #[test]
    fn engine_editions_name_azure_services() {
        assert_eq!(flavour("3"), None);
        assert_eq!(flavour("5"), Some("Azure SQL Database"));
        assert_eq!(flavour("8"), Some("Azure SQL Managed Instance"));
    }
}
