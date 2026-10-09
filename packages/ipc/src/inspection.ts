// SPDX-License-Identifier: Apache-2.0
import { commands } from './generated/bindings';
import type {
  CatalogFeaturesDto,
  ExtensionDto,
  GrantDto,
  ObjectDetailDto,
  ObjectIdentityDto,
  RoleDto,
  RoleIdentityDto,
} from './generated/bindings';
import { unwrap } from './connections';

/** Browsable kinds and admin catalogs of the connected server. */
export function catalogCapabilities(
  profileId: string,
  database: string,
): Promise<CatalogFeaturesDto> {
  return unwrap(commands.catalogCapabilities(profileId, database));
}

/** Inspect one object without executing its stored definition. */
export function inspectObject(
  profileId: string,
  database: string,
  object: ObjectIdentityDto,
): Promise<ObjectDetailDto> {
  return unwrap(commands.catalogObjectDetail(profileId, database, object));
}

/** Installed extensions of this database. */
export function listExtensions(
  profileId: string,
  database: string,
): Promise<ExtensionDto[]> {
  return unwrap(commands.catalogExtensions(profileId, database));
}

/** Public attributes and direct memberships of users and roles. */
export function listRoles(
  profileId: string,
  database: string,
): Promise<RoleDto[]> {
  return unwrap(commands.catalogRoles(profileId, database));
}

/** Explicit grants of the selected account, with no account mutation. */
export function roleGrants(
  profileId: string,
  database: string,
  role: RoleIdentityDto,
): Promise<GrantDto[]> {
  return unwrap(commands.catalogRoleGrants(profileId, database, role));
}
