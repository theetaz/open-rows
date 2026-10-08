// SPDX-License-Identifier: Apache-2.0
//! The driver registry: the only place that names driver crates. Every
//! command resolves a profile's engine here (docs/05 §2).
use openrows_driver_api::Driver;
use openrows_driver_postgres::PgDriver;
use openrows_store::profile::Driver as Engine;

use super::error::{IpcError, IpcErrorKind};

static POSTGRES: PgDriver = PgDriver;

/// The driver serving `engine`.
///
/// # Errors
/// `Unsupported` when no driver serves the engine yet.
pub(crate) fn driver(engine: Engine) -> Result<&'static dyn Driver, IpcError> {
    match engine {
        Engine::Postgres => Ok(&POSTGRES),
        other => Err(IpcError::new(
            IpcErrorKind::Unsupported,
            format!("{} connections are not available yet", other.name()),
        )),
    }
}

/// The driver of saved connection `profile_id`.
///
/// # Errors
/// Unknown profiles, or engines without a driver.
pub(crate) fn profile_driver(
    state: &super::Connections,
    profile_id: &str,
) -> Result<&'static dyn Driver, IpcError> {
    let profile = state
        .profiles
        .get(super::profiles::profile_id(profile_id)?)?;
    driver(profile.driver)
}
