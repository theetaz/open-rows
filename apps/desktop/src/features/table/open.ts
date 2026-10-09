// SPDX-License-Identifier: Apache-2.0
import type { ObjectKindDto } from '@openrows/ipc';
import type { QueryTabs, TabScope } from '../query';

/** Object kinds that open as a table view rather than a query tab. */
const browsable = new Set<ObjectKindDto>([
  'table',
  'view',
  'materializedView',
  'foreignTable',
]);

/** A tree or ⌘P object: `[database, schema, name]` plus its kind. */
type BrowsableObject = {
  database: string;
  schema: string;
  name: string;
  kind: ObjectKindDto;
};

/**
 * Open (or refocus) the table tab of `object` in `scope`, one tab per
 * relation, titled with its name. A `preview` open (single click) replaces
 * the scope's preview tab; any other open keeps the tab. Returns `false` for
 * objects that are not table-like (routines, sequences), which keep opening
 * as query tabs.
 */
export function openTableView(
  tabs: QueryTabs,
  scope: TabScope,
  object: BrowsableObject,
  preview = false,
): boolean {
  if (!browsable.has(object.kind)) return false;
  const open = tabs
    .tabsFor(scope)
    .find(
      ({ table }) =>
        table?.database === object.database &&
        table.schema === object.schema &&
        table.name === object.name,
    );
  if (open) {
    tabs.select(open.id);
    if (!preview) tabs.keep(open.id);
  } else
    tabs.open(scope, {
      preview,
      title: object.name,
      objectPath: [object.schema, object.name],
      table: {
        ...(object.kind === 'table' ? {} : { kind: object.kind }),
        database: object.database,
        schema: object.schema,
        name: object.name,
      },
    });
  return true;
}
