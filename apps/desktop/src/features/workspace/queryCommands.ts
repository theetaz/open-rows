// SPDX-License-Identifier: Apache-2.0
import type { QueryTabs, TabScope } from '../query';
import type { ObjectRef } from '../schema';

/** A double-quoted SQL identifier. */
function quoteIdent(name: string): string {
  return `"${name.replaceAll('"', '""')}"`;
}

/** Relations a plain `SELECT *` reads; routines open an empty tab. */
const selectable = new Set<ObjectRef['kind']>([
  'table',
  'view',
  'materializedView',
  'foreignTable',
  'sequence',
]);

/** Title, path and starting SQL of the tab that shows `object`. */
export function objectTab(object: ObjectRef): {
  title: string;
  objectPath: readonly string[];
  sql: string;
} {
  const title =
    object.arguments === null
      ? object.name
      : `${object.name}(${object.arguments})`;
  return {
    title,
    objectPath: [object.schema, title],
    sql: selectable.has(object.kind)
      ? `SELECT * FROM ${quoteIdent(object.schema)}.${quoteIdent(object.name)} LIMIT 1000;`
      : '',
  };
}

/**
 * Shell actions on query tabs. `scope` is the browsed connection and
 * database; `scopeOf` maps an object's database to its tab scope.
 */
export function queryCommands(
  tabs: QueryTabs,
  scope: TabScope | undefined,
  scopeOf: (database: string) => TabScope | undefined,
): {
  addQuery: (sql?: string) => void;
  openObject: (object: ObjectRef) => void;
  closeTab: () => void;
} {
  return {
    addQuery: (sql) => {
      if (scope) tabs.open(scope, sql === undefined ? {} : { sql });
    },
    openObject: (object) => {
      const target = scopeOf(object.database);
      if (!target) return;
      const init = objectTab(object);
      const path = init.objectPath.join('\n');
      const open = tabs
        .tabsFor(target)
        .find(({ objectPath }) => objectPath?.join('\n') === path);
      if (open) tabs.select(open.id);
      else tabs.open(target, init);
    },
    closeTab: () => {
      const tab = scope ? tabs.activeFor(scope) : undefined;
      if (tab) tabs.close(tab.id);
    },
  };
}
