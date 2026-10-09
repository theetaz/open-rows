// SPDX-License-Identifier: Apache-2.0
import type { SchemaDto } from '@openrows/ipc';
import { objectGroups } from './catalog';
import type { ObjectRef } from './catalog';
import type { ObjectPage } from './useCatalog';
import type { Resource } from './useResources';

type Loaded = {
  schemas: Resource<SchemaDto[]>;
  pages: [string, Resource<ObjectPage>][];
};
const identityKey = (
  object: Pick<ObjectRef, 'schema' | 'kind' | 'name' | 'arguments'>,
): string =>
  JSON.stringify([object.schema, object.kind, object.name, object.arguments]);

/** Build availability once per catalog render, reusing normal settled section reads. */
export function catalogAvailability(
  loaded: Loaded,
): (object: ObjectRef) => boolean | null {
  const present = new Set<string>();
  const complete = new Set<string>();
  for (const [key, page] of settledPages(loaded.pages)) {
    page.objects.forEach((object) => present.add(identityKey(object)));
    if (page.objects.length < page.total) continue;
    const section = key.lastIndexOf('\n');
    const group = key.slice(section + 1);
    complete.add(JSON.stringify([key.slice(0, section), group]));
  }
  const schemas =
    loaded.schemas.status === 'ready' && !loaded.schemas.refreshing
      ? new Set(loaded.schemas.data.map((schema) => schema.name))
      : null;
  return (object) => {
    if (schemas && !schemas.has(object.schema)) return false;
    if (present.has(identityKey(object))) return true;
    const group = objectGroups.find((item) => item.kinds.includes(object.kind));
    return complete.has(JSON.stringify([object.schema, group?.id]))
      ? false
      : null;
  };
}

/** Check a single identity; unread, refreshing and incomplete scopes stay unknown. */
export function objectAvailability(
  object: ObjectRef,
  loaded: Loaded,
): boolean | null {
  return catalogAvailability(loaded)(object);
}

/** Index only the largest settled page of a section, avoiding repeated prefixes. */
function settledPages(pages: Loaded['pages']): Map<string, ObjectPage> {
  const selected = new Map<string, ObjectPage>();
  for (const [key, page] of pages) {
    if (page.status !== 'ready' || page.refreshing) continue;
    const scope = key.slice(0, key.lastIndexOf('\n'));
    const existing = selected.get(scope);
    if (!existing || page.data.objects.length >= existing.objects.length)
      selected.set(scope, page.data);
  }
  return selected;
}
