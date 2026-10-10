// SPDX-License-Identifier: Apache-2.0
import type { InspectionTarget } from '../inspection/target';
import type { ObjectIdentityDto } from '@openrows/ipc';
const kinds = [
  'table',
  'foreignTable',
  'view',
  'materializedView',
  'function',
  'procedure',
  'trigger',
  'sequence',
  'type',
];
/** Validate the inspector identity without accepting arbitrary persisted data. */
export function inspectionOf(value: unknown): InspectionTarget | undefined {
  if (typeof value !== 'object' || value === null) return undefined;
  const item = value as Record<string, unknown>;
  if (typeof item.name === 'string') {
    if (item.kind === 'extension')
      return { kind: 'extension', name: item.name };
    if (item.kind === 'role' && typeof item.scope === 'string')
      return { kind: 'role', name: item.name, scope: item.scope };
  }
  if (
    item.kind !== 'object' ||
    typeof item.object !== 'object' ||
    item.object === null
  )
    return undefined;
  const object = item.object as Record<string, unknown>;
  if (
    typeof object.schema !== 'string' ||
    typeof object.name !== 'string' ||
    !kinds.includes(String(object.kind))
  )
    return undefined;
  if (object.arguments !== null && typeof object.arguments !== 'string')
    return undefined;
  return {
    kind: 'object',
    object: {
      schema: object.schema,
      name: object.name,
      kind: object.kind as ObjectIdentityDto['kind'],
      arguments: object.arguments,
    },
  };
}
