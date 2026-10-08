// SPDX-License-Identifier: Apache-2.0
//! Product, version and flavour from the greeting and one identity query.
use openrows_driver_api::{DriverError, ServerSummary, TlsStatus};

use crate::rows::TextRows;
use crate::wire::Wire;

const INFO_SQL: &str = "SELECT DATABASE(), CURRENT_USER(), VERSION(), @@version_comment";

/// Read the identity of the logged-in session.
pub(crate) async fn fetch(
    wire: &mut Wire,
    greeting_version: &str,
    tls: TlsStatus,
) -> Result<ServerSummary, DriverError> {
    let mut rows = TextRows::default();
    wire.query(INFO_SQL, &mut rows).await?;
    let row = rows.0.into_iter().next().unwrap_or_default();
    let text = |index: usize| row.get(index).cloned().flatten().unwrap_or_default();
    let version = Some(text(2))
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| greeting_version.to_owned());
    let (product, number, flavour) = identify(&version, &text(3));
    Ok(ServerSummary {
        product: product.to_owned(),
        version: number,
        flavour: flavour.map(str::to_owned),
        database: text(0),
        user: text(1),
        tls,
    })
}

/// `(product, version number, flavour)` from `VERSION()` and `@@version_comment`.
pub(crate) fn identify(
    version: &str,
    comment: &str,
) -> (&'static str, String, Option<&'static str>) {
    if version.contains("MariaDB") {
        // `5.5.5-10.11.6-MariaDB-1:10.11.6+maria~ubu2204` from old clients' view.
        let trimmed = version.strip_prefix("5.5.5-").unwrap_or(version);
        let number = trimmed.split('-').next().unwrap_or(trimmed).to_owned();
        return ("MariaDB", number, None);
    }
    let number = version.split('-').next().unwrap_or(version).to_owned();
    let flavour = if version.contains("TiDB") {
        Some("TiDB")
    } else if comment.contains("PlanetScale") {
        Some("PlanetScale")
    } else if version.contains("Vitess") || comment.contains("Vitess") {
        Some("Vitess")
    } else {
        None
    };
    ("MySQL", number, flavour)
}

#[cfg(test)]
mod tests {
    use super::identify;

    #[test]
    fn products_and_flavours_come_from_the_version_strings() {
        assert_eq!(
            identify("8.4.3", "MySQL Community Server - GPL"),
            ("MySQL", "8.4.3".to_owned(), None)
        );
        assert_eq!(
            identify("5.5.5-10.11.6-MariaDB-1:10.11.6+maria~ubu2204", ""),
            ("MariaDB", "10.11.6".to_owned(), None)
        );
        assert_eq!(
            identify("11.4.2-MariaDB-ubu2404", "mariadb.org binary distribution"),
            ("MariaDB", "11.4.2".to_owned(), None)
        );
        assert_eq!(
            identify("8.0.11-TiDB-v7.5.0", ""),
            ("MySQL", "8.0.11".to_owned(), Some("TiDB"))
        );
        assert_eq!(
            identify("8.0.34-Vitess", "PlanetScale"),
            ("MySQL", "8.0.34".to_owned(), Some("PlanetScale"))
        );
    }
}
