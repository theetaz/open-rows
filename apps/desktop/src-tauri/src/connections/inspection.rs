// SPDX-License-Identifier: Apache-2.0
//! Feature-gated read-only inspection on the existing metadata connections.
use super::Connections;
use super::catalog::read_catalog;
use super::drivers::driver;
use super::error::{IpcError, IpcErrorKind};
use super::inspection_dto::{
    CatalogFeaturesDto, ExtensionDto, GrantDto, ObjectDetailDto, ObjectIdentityDto, RoleDto,
    RoleIdentityDto,
};
use super::profiles::profile_id;
use super::route::profile_target;
use openrows_driver_api::catalog::ObjectKind;
use openrows_driver_api::{Capabilities, Feature, ServerSummary};
use tauri::State;

fn capability(state: &Connections, id: &str, feature: Feature) -> Result<Capabilities, IpcError> {
    let profile = state.profiles.get(profile_id(id)?)?;
    let caps = driver(profile.driver).capabilities();
    if !caps.supports(feature) {
        return Err(IpcError::new(
            IpcErrorKind::Unsupported,
            "This engine does not support this catalog",
        ));
    }
    Ok(caps)
}

fn supported(caps: Capabilities, server: &ServerSummary) -> CatalogFeaturesDto {
    let kinds = caps.object_kinds.iter().copied().filter(|kind| {
        if *kind == ObjectKind::Type { return caps.supports(Feature::Types); }
        if *kind == ObjectKind::Sequence && server.product == "MySQL" { return false; }
        if *kind == ObjectKind::Sequence && server.product == "MariaDB" {
            let parts: Vec<_> = server.version.split('.').filter_map(|part| part.parse::<u32>().ok()).collect();
            return matches!(parts.as_slice(), [major, minor, ..] if (*major, *minor) >= (10, 3));
        }
        true
    }).map(Into::into).collect();
    CatalogFeaturesDto {
        kinds,
        extensions: caps.supports(Feature::Extensions),
        roles: caps.supports(Feature::Roles),
    }
}

/// Browsable kinds and admin catalogs, adjusted to the connected server.
#[tauri::command]
#[specta::specta]
pub(crate) async fn catalog_capabilities(
    state: State<'_, Connections>,
    profile_id: String,
    database: String,
) -> Result<CatalogFeaturesDto, IpcError> {
    let caps = capability(&state, &profile_id, Feature::Catalog)?;
    if let Some(shared) = super::duckdb_facets(&state, &profile_id).await? {
        return Ok(supported(caps, shared.server()));
    }
    state
        .metadata
        .read(
            (&profile_id, &database),
            || profile_target(&state, &profile_id),
            move |connection| {
                let features = supported(caps, connection.server());
                Box::pin(async move { Ok(features) })
            },
        )
        .await
}

/// Inspect a schema object on the database's metadata connection.
#[tauri::command]
#[specta::specta]
pub(crate) async fn catalog_object_detail(
    state: State<'_, Connections>,
    profile_id: String,
    database: String,
    object: ObjectIdentityDto,
) -> Result<ObjectDetailDto, IpcError> {
    let caps = capability(&state, &profile_id, Feature::Catalog)?;
    if !caps.lists(object.kind.into())
        || (object.kind == super::catalog_dto::ObjectKindDto::Type
            && !caps.supports(Feature::Types))
    {
        return Err(IpcError::new(
            IpcErrorKind::Unsupported,
            "This engine does not list this object kind",
        ));
    }
    let object = object.into();
    read_catalog(&state, (&profile_id, &database), move |catalog| {
        Box::pin(async move { catalog.object_detail(&object).await })
    })
    .await
    .map(Into::into)
}

/// Installed extensions, without installing or altering anything.
#[tauri::command]
#[specta::specta]
pub(crate) async fn catalog_extensions(
    state: State<'_, Connections>,
    profile_id: String,
    database: String,
) -> Result<Vec<ExtensionDto>, IpcError> {
    capability(&state, &profile_id, Feature::Extensions)?;
    let found = read_catalog(&state, (&profile_id, &database), |catalog| {
        catalog.list_extensions()
    })
    .await?;
    Ok(found.into_iter().map(Into::into).collect())
}

/// Public role/user attributes and memberships, never credential data.
#[tauri::command]
#[specta::specta]
pub(crate) async fn catalog_roles(
    state: State<'_, Connections>,
    profile_id: String,
    database: String,
) -> Result<Vec<RoleDto>, IpcError> {
    capability(&state, &profile_id, Feature::Roles)?;
    let found = read_catalog(&state, (&profile_id, &database), |catalog| {
        catalog.list_roles()
    })
    .await?;
    Ok(found.into_iter().map(Into::into).collect())
}

/// Explicit privileges of the selected scoped account, read-only.
#[tauri::command]
#[specta::specta]
pub(crate) async fn catalog_role_grants(
    state: State<'_, Connections>,
    profile_id: String,
    database: String,
    role: RoleIdentityDto,
) -> Result<Vec<GrantDto>, IpcError> {
    capability(&state, &profile_id, Feature::Roles)?;
    let grants = read_catalog(&state, (&profile_id, &database), move |catalog| {
        Box::pin(async move { catalog.role_grants((&role.name, &role.scope)).await })
    })
    .await?;
    Ok(grants.into_iter().map(Into::into).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use openrows_store::profile::Driver;

    #[test]
    fn libsql_flavours_expose_views_without_unsupported_admin_facets() {
        for engine in [Driver::Libsql, Driver::LibsqlFile] {
            let selected = driver(engine);
            let server = ServerSummary {
                product: selected.product().into(),
                version: "test".into(),
                flavour: None,
                database: "main".into(),
                user: String::new(),
                tls: openrows_driver_api::TlsStatus::Disabled,
            };
            let features = supported(selected.capabilities(), &server);
            assert_eq!(
                features.kinds,
                [
                    super::super::catalog_dto::ObjectKindDto::Table,
                    super::super::catalog_dto::ObjectKindDto::View
                ]
            );
            assert!(!features.extensions);
            assert!(!features.roles);
        }
    }
}
