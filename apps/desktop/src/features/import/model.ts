// SPDX-License-Identifier: Apache-2.0
import type {
  BadRowsDto,
  ConflictDto,
  ImportColumnDto,
  ImportMappingDto,
  ImportOptionsDto,
  ImportPreviewDto,
  ImportRequest,
  TableColumnDto,
  TransactionDto,
} from '@openrows/ipc';

/** Where the import writes: a connected database and schema, maybe a table. */
export type ImportContext = {
  profileId: string;
  sessionId: string;
  database: string;
  schema: string;
  /** The table the sheet was opened for; `null` from File ▸ Import…. */
  table: string | null;
  /** The connection refuses writes. */
  readOnly: boolean;
};

/** The file the user picked; its path stays in the core. */
export type PickedFile = { fileId: string; fileName: string };

/** A target column's source in the mapping of an existing table. */
export type SourceChoice =
  | { kind: 'column'; index: number }
  | { kind: 'skip' }
  | { kind: 'null' }
  | { kind: 'constant'; value: string };

/** One column of a new table, from one source column. */
export type NewColumn = {
  include: boolean;
  name: string;
  type: string;
  key: boolean;
};

/** Everything the sheet lets the user choose. */
export type ImportDraft = {
  file: PickedFile | null;
  options: ImportOptionsDto | null;
  target: 'existing' | 'new';
  table: string;
  /** Existing table: source per target column name. */
  mapping: Record<string, SourceChoice>;
  /** Existing table: key columns of conflict handling and the dry run. */
  key: string[];
  /** New table: one entry per source column. */
  columns: NewColumn[];
  conflict: ConflictDto;
  truncate: boolean;
  transaction: TransactionDto;
  badRows: BadRowsDto;
  /** Rows per batch, as typed. */
  batchSize: string;
};

/** Rows per batch unless the user changes it. */
const DEFAULT_BATCH = '5000';
const MAX_BATCH = 100_000;

/** A fresh draft for `context`: its table when one was given, else a new table. */
export function initialDraft(context: ImportContext): ImportDraft {
  return {
    file: null,
    options: null,
    target: context.table ? 'existing' : 'new',
    table: context.table ?? '',
    mapping: {},
    key: [],
    columns: [],
    conflict: 'error',
    truncate: false,
    transaction: 'single',
    badRows: 'skip',
    batchSize: DEFAULT_BATCH,
  };
}

/** Lower-case letters and digits only, so `First Name` matches `first_name`. */
const loose = (name: string): string =>
  name.toLowerCase().replace(/[^a-z0-9]/g, '');

/** Each target column takes the source column of the same (loose) name. */
export function defaultMapping(
  targets: readonly TableColumnDto[],
  sources: readonly ImportColumnDto[],
): Record<string, SourceChoice> {
  const mapping: Record<string, SourceChoice> = {};
  for (const target of targets) {
    const index = sources.findIndex(
      (source) => loose(source.name) === loose(target.name),
    );
    mapping[target.name] =
      index >= 0 ? { kind: 'column', index } : { kind: 'skip' };
  }
  return mapping;
}

/** A new table takes every source column with its suggested type. */
export function defaultColumns(
  sources: readonly ImportColumnDto[],
): NewColumn[] {
  return sources.map((source) => ({
    include: true,
    name: source.name.trim(),
    type: source.suggestedType,
    key: false,
  }));
}

/** The target type each source column feeds, for preview highlighting. */
export function previewTypes(
  draft: ImportDraft,
  targets: readonly TableColumnDto[],
  sources: number,
): (string | null)[] {
  const types: (string | null)[] = Array.from({ length: sources }, () => null);
  if (draft.target === 'new') {
    draft.columns.forEach((column, index) => {
      if (column.include && index < sources) types[index] = column.type;
    });
    return types;
  }
  for (const target of targets) {
    const choice = draft.mapping[target.name];
    if (choice?.kind === 'column' && choice.index < sources)
      types[choice.index] = target.dataType;
  }
  return types;
}

function existingColumns(draft: ImportDraft): ImportMappingDto[] {
  return Object.entries(draft.mapping).flatMap(
    ([target, choice]): ImportMappingDto[] => {
      switch (choice.kind) {
        case 'column':
          return [{ target, source: { kind: 'column', index: choice.index } }];
        case 'constant':
          return [
            { target, source: { kind: 'constant', value: choice.value } },
          ];
        case 'null':
          return [{ target, source: { kind: 'constant', value: null } }];
        case 'skip':
          return [];
      }
    },
  );
}

function newColumns(draft: ImportDraft): ImportMappingDto[] {
  return draft.columns.flatMap((column, index) =>
    column.include
      ? [{ target: column.name.trim(), source: { kind: 'column', index } }]
      : [],
  );
}

/** Why the draft cannot run yet; `null` when it can. */
export function draftProblem(draft: ImportDraft): string | null {
  if (!draft.file || !draft.options) return 'Choose a file to import.';
  if (!draft.table.trim()) return 'Enter the table name.';
  const batch = Number(draft.batchSize);
  if (!Number.isInteger(batch) || batch < 1 || batch > MAX_BATCH)
    return `Rows per batch must be between 1 and ${MAX_BATCH.toLocaleString()}.`;
  if (draft.target === 'new') return newTableProblem(draft);
  if (existingColumns(draft).length === 0) return 'Map at least one column.';
  if (draft.conflict !== 'error' && draft.key.length === 0)
    return 'Choose the key columns that identify existing rows.';
  return null;
}

function newTableProblem(draft: ImportDraft): string | null {
  const included = draft.columns.filter((column) => column.include);
  if (included.length === 0) return 'Include at least one column.';
  const names = included.map((column) => column.name.trim());
  if (names.some((name) => !name)) return 'Every column needs a name.';
  if (new Set(names).size !== names.length) return 'Column names must differ.';
  if (included.some((column) => !column.type.trim()))
    return 'Every column needs a type.';
  return null;
}

/** The import request for the draft (check it with `draftProblem` first). */
export function importRequest(
  draft: ImportDraft,
  context: ImportContext,
  mode: ImportRequest['mode'],
): ImportRequest | null {
  if (!draft.file || !draft.options || draftProblem(draft)) return null;
  const created = draft.target === 'new';
  const included = draft.columns.filter((column) => column.include);
  return {
    sessionId: context.sessionId,
    target: {
      profileId: context.profileId,
      database: context.database,
      schema: context.schema,
      table: draft.table.trim(),
    },
    fileId: draft.file.fileId,
    options: draft.options,
    columns: created ? newColumns(draft) : existingColumns(draft),
    create: created
      ? {
          types: included.map((column) => column.type.trim()),
          primaryKey: included
            .filter((column) => column.key)
            .map((column) => column.name.trim()),
        }
      : null,
    key: created ? [] : draft.key,
    conflict: created ? 'error' : draft.conflict,
    truncate: !created && draft.truncate,
    transaction: draft.transaction,
    badRows: draft.badRows,
    mode,
    batchSize: Number(draft.batchSize),
  };
}

/** Preview cells to highlight, as `row:column` keys. */
export function invalidCells(preview: ImportPreviewDto): Set<string> {
  return new Set(
    preview.invalid.map((cell) => `${String(cell.row)}:${String(cell.column)}`),
  );
}
