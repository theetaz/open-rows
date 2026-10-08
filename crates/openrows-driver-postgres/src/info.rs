// SPDX-License-Identifier: Apache-2.0
//! Server identity after connect, for the connection test and status bar.
use crate::simple::{self, TextRow};
use crate::startup::PgConfig;
use crate::wire::Wire;
use openrows_driver_api::DriverError;
use openrows_net::tls::TlsDetails;

/// One round trip; the extension probe makes flavour detection free.
const INFO_SQL: &str = "SELECT current_database(), current_user, version(), \
    (SELECT string_agg(extname, ',') FROM pg_catalog.pg_extension \
     WHERE extname IN ('timescaledb', 'neon', 'supabase_vault'))";

/// `PostgreSQL`-compatible product or hosting flavour, detected cheaply.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Flavour {
    /// Plain `PostgreSQL` (or nothing more specific was detected).
    Postgres,
    /// `CockroachDB` (`version()` names it).
    CockroachDb,
    /// `PostgreSQL` with the `timescaledb` extension in this database.
    Timescale,
    /// Supabase (`supabase_vault` extension or a `supabase.co`/`.com` host).
    Supabase,
    /// Neon (`neon` extension or a `neon.tech` host).
    Neon,
}

impl Flavour {
    /// Product name for display, e.g. `PostgreSQL`.
    #[must_use]
    pub fn product(self) -> &'static str {
        match self {
            Self::CockroachDb => "CockroachDB",
            Self::Postgres | Self::Timescale | Self::Supabase | Self::Neon => "PostgreSQL",
        }
    }
}

/// Whether and how the connection is encrypted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TlsStatus {
    /// `sslmode=disable`, or `allow` and the server accepted plaintext.
    Disabled,
    /// A Unix-domain socket: TLS does not apply.
    LocalSocket,
    /// `sslmode=prefer` and the server does not offer TLS: plaintext. Show
    /// this to the user.
    PlaintextFallback,
    /// Encrypted; the details name protocol, cipher and certificate.
    Encrypted(TlsDetails),
}

/// What the server reported about itself right after connecting.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerInfo {
    /// `server_version` parameter status, e.g. `17.2` or `16.4 (Debian 16.4-1)`.
    pub server_version: String,
    /// Full `version()` text; empty when the server rejected the info query.
    pub version: String,
    /// `current_database()`.
    pub database: String,
    /// `current_user`.
    pub user: String,
    /// Detected flavour; see [`Flavour::product`] for the product name.
    pub flavour: Flavour,
    /// Transport encryption.
    pub tls: TlsStatus,
}

impl ServerInfo {
    /// Product name for display, e.g. `PostgreSQL` or `CockroachDB`.
    #[must_use]
    pub fn product(&self) -> &'static str {
        self.flavour.product()
    }
}

/// Query identity; a server that rejects the query (exotic compatible servers)
/// still connects, with the requested database and user as fallbacks.
pub(crate) async fn fetch(
    wire: &mut Wire,
    config: &PgConfig,
    server_version: String,
    tls: TlsStatus,
) -> Result<ServerInfo, DriverError> {
    let row = match simple::query(wire, INFO_SQL).await {
        Ok(rows) => rows.into_iter().next().unwrap_or_default(),
        Err(DriverError::Server { .. }) => TextRow::new(),
        Err(error) => return Err(error),
    };
    let text = |index: usize| row.get(index).cloned().flatten();
    let version = text(2).unwrap_or_default();
    let extensions = text(3).unwrap_or_default();
    Ok(ServerInfo {
        flavour: detect(&version, &extensions, &config.host),
        server_version,
        version,
        database: text(0).unwrap_or_else(|| config.database.clone()),
        user: text(1).unwrap_or_else(|| config.user.clone()),
        tls,
    })
}

/// `extensions` is the comma-separated subset of probed extension names.
fn detect(version: &str, extensions: &str, host: &str) -> Flavour {
    let has = |name: &str| extensions.split(',').any(|extension| extension == name);
    let host = host.to_ascii_lowercase();
    if version.contains("CockroachDB") {
        Flavour::CockroachDb
    } else if has("timescaledb") {
        Flavour::Timescale
    } else if has("supabase_vault")
        || host.ends_with(".supabase.co")
        || host.ends_with(".supabase.com")
    {
        Flavour::Supabase
    } else if has("neon") || host.ends_with(".neon.tech") {
        Flavour::Neon
    } else {
        Flavour::Postgres
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flavours_come_from_version_extensions_and_host() {
        let pg = "PostgreSQL 17.2 on aarch64-unknown-linux-musl";
        assert_eq!(detect(pg, "", "localhost"), Flavour::Postgres);
        let crdb = "CockroachDB CCL v24.1.0 (aarch64-apple-darwin21.2, built 2024/05/15)";
        assert_eq!(detect(crdb, "", "localhost"), Flavour::CockroachDb);
        assert_eq!(detect(pg, "neon,timescaledb", "h"), Flavour::Timescale);
        assert_eq!(detect(pg, "supabase_vault", "h"), Flavour::Supabase);
        assert_eq!(detect(pg, "", "db.abc.supabase.co"), Flavour::Supabase);
        assert_eq!(
            detect(pg, "", "ep-x.eu-central-1.aws.NEON.tech"),
            Flavour::Neon
        );
        assert_eq!(detect(pg, "neonx", "h"), Flavour::Postgres);
        assert_eq!(Flavour::CockroachDb.product(), "CockroachDB");
        assert_eq!(Flavour::Neon.product(), "PostgreSQL");
    }
}
