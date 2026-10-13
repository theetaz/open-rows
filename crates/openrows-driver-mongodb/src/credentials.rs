// SPDX-License-Identifier: Apache-2.0
//! Authentication defaults stay bound to the profile, not the browsed database.
use mongodb::options::Credential;
use openrows_driver_api::{ConnectRequest, DriverError};

pub(crate) fn credential(
    request: &ConnectRequest,
    discovered: Option<&Credential>,
) -> Result<Option<Credential>, DriverError> {
    if request.user.is_empty() {
        return if request.password.is_some() {
            Err(DriverError::Auth(
                "Enter a MongoDB user or clear the password".into(),
            ))
        } else {
            Ok(None)
        };
    }
    Ok(Some(
        Credential::builder()
            .username(request.user.clone())
            .password(request.password.as_ref().map(|p| p.expose().to_owned()))
            .source(authentication_source(request, discovered))
            .build(),
    ))
}

fn authentication_source(request: &ConnectRequest, discovered: Option<&Credential>) -> String {
    request
        .driver_options
        .get("authSource")
        .or_else(|| discovered.and_then(|c| c.source.as_ref()))
        .or_else(|| request.driver_options.get("mongodb_default_auth_source"))
        .cloned()
        .unwrap_or_else(|| {
            if request.database.is_empty() {
                "admin".into()
            } else {
                request.database.clone()
            }
        })
}

#[cfg(test)]
#[path = "credentials_tests.rs"]
mod tests;
