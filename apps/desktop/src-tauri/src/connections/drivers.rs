// SPDX-License-Identifier: Apache-2.0
//! The driver registry: the only place that names driver crates. Every
//! command resolves a profile's engine here (docs/05 §2).
use openrows_driver_api::Driver;
use openrows_driver_clickhouse::ClickHouseDriver;
use openrows_driver_libsql::LibsqlDriver;
use openrows_driver_mssql::SqlServerDriver;
use openrows_driver_mysql::{Flavour, MySqlDriver};
use openrows_driver_postgres::PgDriver;
use openrows_driver_sqlite::{LibsqlFileDriver, SqliteDriver};
use openrows_store::profile::Driver as Engine;

use super::error::IpcError;

static POSTGRES: PgDriver = PgDriver;
static MYSQL: MySqlDriver = MySqlDriver(Flavour::MySql);
static MARIADB: MySqlDriver = MySqlDriver(Flavour::MariaDb);
static SQLITE: SqliteDriver = SqliteDriver;
static CLICKHOUSE: ClickHouseDriver = ClickHouseDriver;
static MONGODB: openrows_driver_mongodb::MongoDriver = openrows_driver_mongodb::MongoDriver;
static LIBSQL: LibsqlDriver = LibsqlDriver;
static LIBSQL_FILE: LibsqlFileDriver = LibsqlFileDriver;
static SQL_SERVER: SqlServerDriver = SqlServerDriver;
static DUCKDB: openrows_host_client::DuckdbMetadata = openrows_host_client::DuckdbMetadata;

/// The driver serving `engine`; every engine a profile can name has one.
pub(crate) fn driver(engine: Engine) -> &'static dyn Driver {
    match engine {
        Engine::Duckdb => &DUCKDB,
        Engine::Postgres => &POSTGRES,
        Engine::Mysql => &MYSQL,
        Engine::Mariadb => &MARIADB,
        Engine::Sqlserver => &SQL_SERVER,
        Engine::Clickhouse => &CLICKHOUSE,
        Engine::Sqlite => &SQLITE,
        Engine::Libsql => &LIBSQL,
        Engine::Mongodb => &MONGODB,
        Engine::LibsqlFile => &LIBSQL_FILE,
    }
}

/// The driver of saved connection `profile_id`.
///
/// # Errors
/// Unknown profiles.
pub(crate) fn profile_driver(
    state: &super::Connections,
    profile_id: &str,
) -> Result<&'static dyn Driver, IpcError> {
    let profile = state
        .profiles
        .get(super::profiles::profile_id(profile_id)?)?;
    Ok(driver(profile.driver))
}
