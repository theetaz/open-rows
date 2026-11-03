// SPDX-License-Identifier: Apache-2.0
import {
  inspectObject,
  listExtensions,
  listRoles,
  roleGrants,
} from '@openrows/ipc';
import type { ObjectDetailDto } from '@openrows/ipc';
import type { InspectionTarget } from './target';
import { t } from './messages';

/** Read public metadata for the selected catalog identity. */
export async function readInspection(input: {
  profileId: string;
  database: string;
  target: InspectionTarget;
}): Promise<ObjectDetailDto> {
  const { profileId, database, target } = input;
  if (target.kind === 'object')
    return inspectObject(profileId, database, target.object);
  if (target.kind === 'extension') {
    const found = (await listExtensions(profileId, database)).find(
      (item) => item.name === target.name,
    );
    if (!found) throw new Error(t('extensionNoLonger'));
    return {
      attributes: [
        { name: t('name'), value: found.name },
        { name: t('installedVersion'), value: found.version },
        { name: t('defaultAvailableVersion'), value: found.availableVersion },
        { name: t('schema'), value: found.schema },
        { name: t('description'), value: found.description },
      ],
      sections: [],
      definition: null,
    };
  }
  const [roles, grants] = await Promise.all([
    listRoles(profileId, database),
    roleGrants(profileId, database, { name: target.name, scope: target.scope }),
  ]);
  const role = roles.find(
    (item) => item.name === target.name && item.scope === target.scope,
  );
  if (!role) throw new Error(t('accountNoLonger'));
  return {
    attributes: [
      { name: t('name'), value: role.name },
      { name: t('scope'), value: role.scope },
      ...role.attributes,
    ],
    sections: [
      {
        name: t('member'),
        columns: [t('role')],
        rows: role.attributes.some(
          (attribute) => attribute.name === 'Membership visibility',
        )
          ? [[t('notVisibleLogin')]]
          : role.memberships.map((membership) => [membership]),
      },
      {
        name: t('grants'),
        columns: [
          t('scope'),
          t('object'),
          t('privilege'),
          t('state'),
          t('grantor'),
        ],
        rows: grants.map((grant) => [
          grant.scope,
          grant.object,
          grant.privilege,
          grant.state,
          grant.grantor,
        ]),
      },
    ],
    definition: null,
  };
}
