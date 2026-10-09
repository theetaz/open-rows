// SPDX-License-Identifier: Apache-2.0
import {
  inspectObject,
  listExtensions,
  listRoles,
  roleGrants,
} from '@openrows/ipc';
import type { ObjectDetailDto } from '@openrows/ipc';
import type { InspectionTarget } from './target';

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
    if (!found)
      throw new Error('Extension no longer exists or is not visible.');
    return {
      attributes: [
        { name: 'Name', value: found.name },
        { name: 'Installed version', value: found.version },
        { name: 'Default available version', value: found.availableVersion },
        { name: 'Schema', value: found.schema },
        { name: 'Description', value: found.description },
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
  if (!role) throw new Error('Account no longer exists or is not visible.');
  return {
    attributes: [
      { name: 'Name', value: role.name },
      { name: 'Scope', value: role.scope },
      ...role.attributes,
    ],
    sections: [
      {
        name: 'Member of',
        columns: ['Role'],
        rows: role.attributes.some(
          (attribute) => attribute.name === 'Membership visibility',
        )
          ? [['Not visible to this login']]
          : role.memberships.map((membership) => [membership]),
      },
      {
        name: 'Grants',
        columns: ['Scope', 'Object', 'Privilege', 'State', 'Grantor'],
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
