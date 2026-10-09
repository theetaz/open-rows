// SPDX-License-Identifier: Apache-2.0
import type { ObjectKindDto } from '@openrows/ipc';
import type { ObjectRef } from './catalog';

/** Durable catalog identity; routine arguments distinguish overloads. */
export type SidebarObject = ObjectRef & { profileId: string };

const kinds: readonly ObjectKindDto[] = [
  'table',
  'foreignTable',
  'view',
  'materializedView',
  'function',
  'procedure',
  'sequence',
  'type',
];

/** Collision-free key, independent of session generation and catalog order. */
export function sidebarObjectKey(object: SidebarObject): string {
  return JSON.stringify([
    object.profileId,
    object.database,
    object.schema,
    object.kind,
    object.name,
    object.arguments,
  ]);
}

/** Reject malformed local identities instead of guessing their kind or scope. */
export function parseSidebarObject(raw: unknown): SidebarObject | null {
  if (!raw || typeof raw !== 'object') return null;
  const value = raw as Record<string, unknown>;
  if (
    !['profileId', 'database', 'schema', 'name'].every(
      (key) => typeof value[key] === 'string',
    ) ||
    !kinds.includes(value.kind as ObjectKindDto) ||
    !(value.arguments === null || typeof value.arguments === 'string')
  )
    return null;
  return {
    profileId: value.profileId as string,
    database: value.database as string,
    schema: value.schema as string,
    name: value.name as string,
    kind: value.kind as ObjectKindDto,
    arguments: value.arguments,
  };
}

/** Valid identities in first-seen order, deduplicated by their complete key. */
export function parseObjectList(raw: unknown): SidebarObject[] {
  if (!Array.isArray(raw)) return [];
  const unique = new Map<string, SidebarObject>();
  for (const item of raw) {
    const object = parseSidebarObject(item);
    if (object && !unique.has(sidebarObjectKey(object)))
      unique.set(sidebarObjectKey(object), object);
  }
  return [...unique.values()];
}
