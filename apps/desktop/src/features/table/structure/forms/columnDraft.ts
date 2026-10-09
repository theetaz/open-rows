// SPDX-License-Identifier: Apache-2.0
import type {
  DefaultDto,
  EngineDto,
  IdentityDto,
  NewColumnDto,
  SchemaChangeDto,
} from '@openrows/ipc';
import type { ColumnRow } from '../effective';
import { typeSpecs } from './dialect';

/** What the column form edits; sizes stay text until the type is composed. */
export type ColumnDraft = {
  name: string;
  /** Base type without its size, e.g. `varchar` or `numeric`. */
  type: string;
  /** Length, or precision of an exact numeric. */
  length: string;
  /** Scale of an exact numeric. */
  scale: string;
  nullable: boolean;
  defaultMode: DefaultDto['kind'];
  defaultText: string;
  identity: IdentityDto;
  collation: string;
  charset: string;
  comment: string;
};

/** A new nullable text column. */
export function emptyColumn(name: string, engine: EngineDto): ColumnDraft {
  return {
    name,
    type: typeSpecs(engine)[0]?.name ?? 'text',
    length: '',
    scale: '',
    nullable: true,
    defaultMode: 'none',
    defaultText: '',
    identity: 'none',
    collation: '',
    charset: '',
    comment: '',
  };
}

/** `varchar(40)` → `varchar`, `40`; `numeric(10, 2)` → `numeric`, `10`, `2`. */
export function splitType(text: string): {
  type: string;
  length: string;
  scale: string;
} {
  const match = /^([^(]+)\(\s*(\d+)\s*(?:,\s*(\d+)\s*)?\)(.*)$/.exec(
    text.trim(),
  );
  if (!match) return { type: text.trim(), length: '', scale: '' };
  const [, base = '', length = '', scale = '', rest = ''] = match;
  return { type: `${base.trim()}${rest}`.trim(), length, scale };
}

/** The type text the form composes, e.g. `numeric(10,2)`. */
export function columnType(draft: ColumnDraft): string {
  const type = draft.type.trim();
  const length = draft.length.trim();
  if (!length) return type;
  const scale = draft.scale.trim();
  const size = scale ? `${length},${scale}` : length;
  const unsigned = / unsigned$/i.exec(type);
  return unsigned
    ? `${type.slice(0, unsigned.index)}(${size})${unsigned[0]}`
    : `${type}(${size})`;
}

/** The default the form describes. */
function draftDefault(draft: ColumnDraft): DefaultDto {
  switch (draft.defaultMode) {
    case 'literal':
      return { kind: 'literal', value: draft.defaultText };
    case 'expression':
      return draft.defaultText.trim()
        ? { kind: 'expression', sql: draft.defaultText.trim() }
        : { kind: 'none' };
    case 'none':
      return { kind: 'none' };
  }
}

const identityOf: Record<ColumnRow['generated'], IdentityDto> = {
  no: 'none',
  stored: 'none',
  identityAlways: 'always',
  identityByDefault: 'byDefault',
  autoIncrement: 'autoIncrement',
};

/** The form of an existing (or staged new) column. */
export function draftFromRow(row: ColumnRow): ColumnDraft {
  const defaultText = row.default ?? '';
  const literal = /^'(.*)'$/s.exec(defaultText);
  return {
    name: row.name,
    ...splitType(row.dataType),
    nullable: row.nullable,
    defaultMode:
      row.default === null ? 'none' : literal ? 'literal' : 'expression',
    defaultText: literal
      ? (literal[1] ?? '').replaceAll("''", "'")
      : defaultText,
    identity: identityOf[row.generated],
    collation: '',
    charset: '',
    comment: row.comment ?? '',
  };
}

/** Why the form cannot be staged yet, or `null`. */
export function columnProblem(draft: ColumnDraft): string | null {
  if (!draft.name.trim()) return 'Name the column.';
  if (!draft.type.trim()) return 'Choose a type.';
  if (draft.scale.trim() && !draft.length.trim())
    return 'A scale needs a precision.';
  if (draft.identity !== 'none' && draftDefault(draft).kind !== 'none')
    return 'A numbered column has no default.';
  return null;
}

const text = (value: string): string | null => value.trim() || null;

/** The column as part of a new table. */
export function newColumn(draft: ColumnDraft): NewColumnDto {
  return {
    name: draft.name.trim(),
    dataType: columnType(draft),
    nullable: draft.nullable,
    default: draftDefault(draft),
    comment: text(draft.comment),
    identity: draft.identity,
    collation: text(draft.collation),
    charset: text(draft.charset),
  };
}

/** The column as a staged `addColumn`. */
export function addColumnChange(draft: ColumnDraft): SchemaChangeDto {
  return { kind: 'addColumn', ...newColumn(draft) };
}

/** One staged change per property that differs from `original`. */
export function columnChanges(
  draft: ColumnDraft,
  column: string,
  original: ColumnDraft,
): SchemaChangeDto[] {
  const changes: SchemaChangeDto[] = [];
  const type = columnType(draft);
  if (type !== columnType(original))
    changes.push({ kind: 'columnType', column, dataType: type });
  if (draft.collation.trim() || draft.charset.trim())
    changes.push({
      kind: 'columnCollation',
      column,
      charset: text(draft.charset),
      collation: text(draft.collation),
    });
  if (draft.nullable !== original.nullable)
    changes.push({ kind: 'columnNullable', column, nullable: draft.nullable });
  const value = draftDefault(draft);
  if (JSON.stringify(value) !== JSON.stringify(draftDefault(original)))
    changes.push({ kind: 'columnDefault', column, default: value });
  if (draft.identity !== original.identity)
    changes.push({ kind: 'columnIdentity', column, identity: draft.identity });
  if (draft.comment !== original.comment)
    changes.push({
      kind: 'columnComment',
      column,
      comment: text(draft.comment),
    });
  if (draft.name.trim() !== column)
    changes.push({ kind: 'renameColumn', column, name: draft.name.trim() });
  return changes;
}
