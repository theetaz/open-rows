// SPDX-License-Identifier: Apache-2.0
import type { ObjectFeaturesDto } from '@openrows/ipc';
import type { ObjectEditorRequest } from './requests';

/** Palette and menu commands that open an object editor. */
export const OBJECT_COMMANDS = [
  'new-table',
  'new-view',
  'new-materialized-view',
  'new-function',
  'new-procedure',
  'new-trigger',
  'rename-table',
  'duplicate-table',
  'truncate-table',
  'drop-table',
  'edit-view',
  'drop-view',
  'new-database',
  'rename-database',
  'drop-database',
  'new-schema',
  'rename-schema',
  'drop-schema',
] as const;
export type ObjectCommand = (typeof OBJECT_COMMANDS)[number];

export function isObjectCommand(id: string): id is ObjectCommand {
  return (OBJECT_COMMANDS as readonly string[]).includes(id);
}

/** What object commands act on: the browsed schema and the active table tab. */
export type CommandContext = {
  /** The open database and selected schema, when one is browsed. */
  place: { database: string; schema: string } | null;
  /** The relation of the active table tab, if any. */
  table: { database: string; schema: string; name: string } | null;
};

/** The feature a command needs. */
const needs: Record<ObjectCommand, keyof ObjectFeaturesDto> = {
  'new-table': 'tables',
  'new-view': 'views',
  'new-materialized-view': 'materializedViews',
  'new-function': 'routines',
  'new-procedure': 'routines',
  'new-trigger': 'triggers',
  'rename-table': 'tables',
  'duplicate-table': 'tables',
  'truncate-table': 'tables',
  'drop-table': 'tables',
  'edit-view': 'views',
  'drop-view': 'views',
  'new-database': 'databases',
  'rename-database': 'renameDatabases',
  'drop-database': 'databases',
  'new-schema': 'schemas',
  'rename-schema': 'schemas',
  'drop-schema': 'schemas',
};

/** Whether the engine supports what `id` opens. */
export function objectCommandSupported(
  id: ObjectCommand,
  features: ObjectFeaturesDto,
): boolean {
  return features[needs[id]];
}

/** Commands on the active table tab's relation. */
function relationRequest(
  id: ObjectCommand,
  table: NonNullable<CommandContext['table']>,
): ObjectEditorRequest | null {
  switch (id) {
    case 'new-trigger':
      return {
        kind: 'trigger',
        database: table.database,
        schema: table.schema,
        table: table.name,
        name: null,
      };
    case 'rename-table':
      return { kind: 'renameTable', ...table };
    case 'duplicate-table':
      return { kind: 'duplicateTable', ...table };
    case 'truncate-table':
      return { kind: 'truncateTable', ...table };
    case 'drop-table':
      return { kind: 'drop', target: { object: 'table', ...table } };
    case 'edit-view':
      return { kind: 'view', ...table, materialized: false };
    case 'drop-view':
      return { kind: 'drop', target: { object: 'view', ...table } };
    default:
      return null;
  }
}

/** Commands on the browsed database and schema. */
function placeRequest(
  id: ObjectCommand,
  place: NonNullable<CommandContext['place']>,
): ObjectEditorRequest | null {
  const { database, schema } = place;
  switch (id) {
    case 'new-table':
      return { kind: 'newTable', ...place };
    case 'new-view':
    case 'new-materialized-view':
      return {
        kind: 'view',
        ...place,
        name: null,
        materialized: id === 'new-materialized-view',
      };
    case 'new-function':
    case 'new-procedure':
      return {
        kind: 'routine',
        ...place,
        routine: id === 'new-function' ? 'function' : 'procedure',
        name: null,
        arguments: null,
      };
    case 'new-database':
      return { kind: 'database', database, name: null };
    case 'rename-database':
      return { kind: 'database', database, name: database };
    case 'drop-database':
      return {
        kind: 'drop',
        target: { object: 'database', database, name: database },
      };
    case 'new-schema':
      return { kind: 'schema', database, name: null };
    case 'rename-schema':
      return schema ? { kind: 'schema', database, name: schema } : null;
    case 'drop-schema':
      return schema
        ? { kind: 'drop', target: { object: 'schema', database, name: schema } }
        : null;
    default:
      return null;
  }
}

/** The editor `id` opens in `context`, or `null` when there is nothing to act on. */
export function objectCommandRequest(
  id: ObjectCommand,
  context: CommandContext,
): ObjectEditorRequest | null {
  const fromTable = context.table ? relationRequest(id, context.table) : null;
  if (fromTable) return fromTable;
  return context.place ? placeRequest(id, context.place) : null;
}
