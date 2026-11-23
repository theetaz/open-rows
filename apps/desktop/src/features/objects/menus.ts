// SPDX-License-Identifier: Apache-2.0
import type {
  ObjectFeaturesDto,
  ObjectKindDto,
  RoutineKindDto,
} from '@openrows/ipc';
import type { ObjectEditorRequest } from './requests';
import { t } from './messages';

/** One menu entry; `request: null` is shown disabled with `reason`. */
type EditorEntry = {
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

const UNSUPPORTED = t('notSupportedDatabase');

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
    entry(t('newTable'), features.tables, { kind: 'newTable', ...place }),
    entry(t('newView'), features.views, {
      kind: 'view',
      ...place,
      name: null,
      materialized: false,
    }),
    entry(t('newMaterializedView'), features.materializedViews, {
      kind: 'view',
      ...place,
      name: null,
      materialized: true,
    }),
    entry(t('newFunction'), features.routines, routine('function')),
    entry(t('newProcedure'), features.routines, routine('procedure')),
  ];
}

/** Rename, duplicate, truncate, drop and new trigger for a table. */
function tableEntries(
  table: Place & { name: string },
  features: ObjectFeaturesDto,
): EditorEntry[] {
  const supported = features.tables;
  return [
    entry(t('renameTable'), supported, { kind: 'renameTable', ...table }),
    entry(t('duplicateTable'), supported, { kind: 'duplicateTable', ...table }),
    entry(t('newTrigger'), features.triggers, {
      kind: 'trigger',
      database: table.database,
      schema: table.schema,
      table: table.name,
      name: null,
    }),
    entry(
      t('truncateTable'),
      supported,
      { kind: 'truncateTable', ...table },
      true,
    ),
    entry(
      t('dropTable'),
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
  const noun = materialized ? t('materializedView2') : t('view');
  return [
    entry(t('editNoun', { noun }), supported, {
      kind: 'view',
      ...view,
      materialized,
    }),
    entry(
      t('dropNoun', { noun }),
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
        entry(t('newTable'), features.tables, { kind: 'newTable', ...place }),
        ...tableEntries(named, features),
      ];
    case 'view':
    case 'materializedView':
      return viewEntries(named, object.kind === 'materializedView', features);
    case 'function':
    case 'procedure': {
      const routine = object.kind;
      const noun = routine === 'function' ? t('function') : t('procedure');
      const target = { ...named, arguments: object.arguments };
      return [
        entry(t('editNoun', { noun }), features.routines, {
          kind: 'routine',
          ...target,
          routine,
        }),
        entry(
          t('dropNoun', { noun }),
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
    entry(t('newSchema2'), features.schemas, {
      kind: 'schema',
      database,
      name: null,
    }),
    entry(t('renameSchema'), features.schemas && Boolean(schema), {
      kind: 'schema',
      database,
      name: schema,
    }),
    entry(
      t('dropSchema'),
      features.schemas && Boolean(schema),
      {
        kind: 'drop',
        target: { object: 'schema', database, name: schema },
      },
      true,
    ),
    entry(t('newDatabase2'), features.databases, {
      kind: 'database',
      database,
      name: null,
    }),
    entry(t('renameDatabase'), features.renameDatabases, {
      kind: 'database',
      database,
      name: database,
    }),
    entry(
      t('dropDatabase'),
      features.databases,
      {
        kind: 'drop',
        target: { object: 'database', database, name: database },
      },
      true,
    ),
  ];
}
