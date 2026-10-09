// SPDX-License-Identifier: Apache-2.0
import type {
  ObjectFeaturesDto,
  ObjectKindDto,
  RoutineKindDto,
} from '@openrows/ipc';
import type { ObjectEditorRequest } from './requests';

/** One menu entry; `request: null` is shown disabled with `reason`. */
export type EditorEntry = {
  label: string;
  request: ObjectEditorRequest | null;
  reason?: string;
  danger?: boolean;
};

/** A tree object: database, schema, name, kind and routine arguments. */
type TreeObject = {
  database: string;
  schema: string;
  name: string;
  kind: ObjectKindDto;
  arguments: string | null;
};

/** Where new objects go: a database and one of its schemas. */
type Place = { database: string; schema: string };

const UNSUPPORTED = 'Not supported by this database';

/** `request` when `supported`, else a disabled entry saying why. */
function entry(
  label: string,
  supported: boolean,
  request: ObjectEditorRequest,
  danger = false,
): EditorEntry {
  return supported
    ? { label, request, danger }
    : { label, request: null, reason: UNSUPPORTED, danger };
}

/** New table, view, materialized view, function and procedure in a schema. */
export function newObjectEntries(
  place: Place,
  features: ObjectFeaturesDto,
): EditorEntry[] {
  const routine = (routine: RoutineKindDto): ObjectEditorRequest => ({
    kind: 'routine',
    ...place,
    routine,
    name: null,
    arguments: null,
  });
  return [
    entry('New Table…', features.tables, { kind: 'newTable', ...place }),
    entry('New View…', features.views, {
      kind: 'view',
      ...place,
      name: null,
      materialized: false,
    }),
    entry('New Materialized View…', features.materializedViews, {
      kind: 'view',
      ...place,
      name: null,
      materialized: true,
    }),
    entry('New Function…', features.routines, routine('function')),
    entry('New Procedure…', features.routines, routine('procedure')),
  ];
}

/** Rename, duplicate, truncate, drop and new trigger for a table. */
function tableEntries(
  table: Place & { name: string },
  features: ObjectFeaturesDto,
): EditorEntry[] {
  const supported = features.tables;
  return [
    entry('Rename Table…', supported, { kind: 'renameTable', ...table }),
    entry('Duplicate Table…', supported, { kind: 'duplicateTable', ...table }),
    entry('New Trigger…', features.triggers, {
      kind: 'trigger',
      database: table.database,
      schema: table.schema,
      table: table.name,
      name: null,
    }),
    entry(
      'Truncate Table…',
      supported,
      { kind: 'truncateTable', ...table },
      true,
    ),
    entry(
      'Drop Table…',
      supported,
      { kind: 'drop', target: { object: 'table', ...table } },
      true,
    ),
  ];
}

/** Edit and drop for a view or materialized view. */
function viewEntries(
  view: Place & { name: string },
  materialized: boolean,
  features: ObjectFeaturesDto,
): EditorEntry[] {
  const supported = materialized ? features.materializedViews : features.views;
  const noun = materialized ? 'Materialized View' : 'View';
  return [
    entry(`Edit ${noun}…`, supported, { kind: 'view', ...view, materialized }),
    entry(
      `Drop ${noun}…`,
      supported,
      {
        kind: 'drop',
        target: { object: materialized ? 'materializedView' : 'view', ...view },
      },
      true,
    ),
  ];
}

/** The lifecycle entries of a tree object's context menu. */
export function objectEntries(
  object: TreeObject,
  features: ObjectFeaturesDto,
): EditorEntry[] {
  const place = { database: object.database, schema: object.schema };
  const named = { ...place, name: object.name };
  switch (object.kind) {
    case 'table':
      return [
        entry('New Table…', features.tables, { kind: 'newTable', ...place }),
        ...tableEntries(named, features),
      ];
    case 'view':
    case 'materializedView':
      return viewEntries(named, object.kind === 'materializedView', features);
    case 'function':
    case 'procedure': {
      const routine = object.kind;
      const noun = routine === 'function' ? 'Function' : 'Procedure';
      const target = { ...named, arguments: object.arguments };
      return [
        entry(`Edit ${noun}…`, features.routines, {
          kind: 'routine',
          ...target,
          routine,
        }),
        entry(
          `Drop ${noun}…`,
          features.routines,
          { kind: 'drop', target: { object: routine, ...target } },
          true,
        ),
      ];
    }
    default:
      return [];
  }
}

/** New, rename and drop of the selected schema and database. */
export function containerEntries(
  place: Place,
  features: ObjectFeaturesDto,
): EditorEntry[] {
  const { database, schema } = place;
  return [
    entry('New Schema…', features.schemas, {
      kind: 'schema',
      database,
      name: null,
    }),
    entry('Rename Schema…', features.schemas && Boolean(schema), {
      kind: 'schema',
      database,
      name: schema,
    }),
    entry(
      'Drop Schema…',
      features.schemas && Boolean(schema),
      {
        kind: 'drop',
        target: { object: 'schema', database, name: schema },
      },
      true,
    ),
    entry('New Database…', features.databases, {
      kind: 'database',
      database,
      name: null,
    }),
    entry('Rename Database…', features.renameDatabases, {
      kind: 'database',
      database,
      name: database,
    }),
    entry(
      'Drop Database…',
      features.databases,
      {
        kind: 'drop',
        target: { object: 'database', database, name: database },
      },
      true,
    ),
  ];
}
