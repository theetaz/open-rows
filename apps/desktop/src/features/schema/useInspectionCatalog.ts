// SPDX-License-Identifier: Apache-2.0
import { useEffect } from 'react';
import { catalogCapabilities, listExtensions, listRoles } from '@openrows/ipc';
import type { CatalogFeaturesDto, ExtensionDto, RoleDto } from '@openrows/ipc';
import type { Resource, Resources } from './useResources';

/** Feature claims and lazy database/connection administration listings. */
export type InspectionCatalog = {
  capabilities: Resource<CatalogFeaturesDto>;
  extensions: Resource<ExtensionDto[]>;
  roles: Resource<RoleDto[]>;
  load: (kind: 'extensions' | 'roles') => void;
};

/** Only capabilities load immediately; account/extension rows wait for disclosure. */
export function useInspectionCatalog(
  resources: Resources,
  prefix: string,
  target: { profileId: string | null; database: string },
): InspectionCatalog {
  const { profileId, database } = target;
  const key = `${prefix}capabilities`;
  const capabilities = resources.get<CatalogFeaturesDto>(key);
  const { load } = resources;
  useEffect(() => {
    if (!profileId || !database) return;
    load(key, () =>
      catalogCapabilities(profileId, database).then((data) => ({
        data,
        stale: false,
      })),
    );
  }, [load, key, profileId, database]);
  return {
    capabilities,
    extensions: resources.get(`${prefix}extensions`),
    roles: resources.get(`${prefix}roles`),
    load: (kind) => {
      if (
        !profileId ||
        !database ||
        capabilities.status !== 'ready' ||
        !capabilities.data[kind]
      )
        return;
      const read = kind === 'roles' ? listRoles : listExtensions;
      load(`${prefix}${kind}`, () =>
        read(profileId, database).then((data) => ({ data, stale: false })),
      );
    },
  };
}
