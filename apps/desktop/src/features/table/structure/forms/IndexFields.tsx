// SPDX-License-Identifier: Apache-2.0
import { SelectField, TextField } from '@openrows/ui';
import type {
  EngineDto,
  IndexMethodDto,
  NewIndexDto,
  ObjectFeaturesDto,
} from '@openrows/ipc';
import { indexMethods } from './dialect';
import styles from './Forms.module.css';

/** What the index form edits. */
export type IndexDraft = NewIndexDto;

/** An unnamed B-tree index without keys. */
export const emptyIndex: IndexDraft = {
  name: null,
  columns: [],
  unique: false,
  method: 'btree',
  descending: [],
  predicate: null,
};

/** Why the index cannot be staged yet, or `null`. */
export function indexProblem(draft: IndexDraft): string | null {
  return draft.columns.length ? null : 'Choose at least one key column.';
}

/** The draft as sent: blank name and predicate are none. */
export function cleanIndex(draft: IndexDraft): IndexDraft {
  return {
    ...draft,
    name: draft.name?.trim() || null,
    predicate: draft.predicate?.trim() || null,
  };
}

const orders = [
  { value: 'asc', label: 'ASC' },
  { value: 'desc', label: 'DESC' },
] as const;

/**
 * Name, key columns in the order chosen with ASC/DESC each, unique,
 * method and (where supported) a partial-index predicate.
 */
export function IndexFields({
  draft,
  onChange,
  columns,
  engine,
  features,
}: {
  draft: IndexDraft;
  onChange: (draft: IndexDraft) => void;
  /** Column names the index may use. */
  columns: readonly string[];
  engine: EngineDto;
  features: ObjectFeaturesDto;
}): React.JSX.Element {
  const set = (patch: Partial<IndexDraft>): void => {
    onChange({ ...draft, ...patch });
  };
  return (
    <div className={styles.grid}>
      <TextField
        label="Name"
        value={draft.name ?? ''}
        placeholder="Automatic"
        autoComplete="off"
        onChange={(event) => {
          set({ name: event.target.value });
        }}
      />
      <SelectField
        label="Algorithm"
        value={draft.method}
        options={indexMethods(engine)}
        onValueChange={(method: IndexMethodDto) => {
          set({ method });
        }}
      />
      <KeyColumns draft={draft} columns={columns} onChange={onChange} />
      <label className={styles.checkRow}>
        <input
          type="checkbox"
          checked={draft.unique}
          onChange={(event) => {
            set({ unique: event.target.checked });
          }}
        />
        Unique
      </label>
      {features.partialIndexes && (
        <TextField
          label="Condition (partial index)"
          className={styles.wide}
          placeholder="deleted_at IS NULL"
          value={draft.predicate ?? ''}
          spellCheck={false}
          onChange={(event) => {
            set({ predicate: event.target.value });
          }}
        />
      )}
    </div>
  );
}

/** Key columns as checkboxes in click order, each with its sort order. */
function KeyColumns({
  draft,
  columns,
  onChange,
}: {
  draft: IndexDraft;
  columns: readonly string[];
  onChange: (draft: IndexDraft) => void;
}): React.JSX.Element {
  const toggle = (column: string): void => {
    const chosen = draft.columns.includes(column);
    onChange({
      ...draft,
      columns: chosen
        ? draft.columns.filter((item) => item !== column)
        : [...draft.columns, column],
      descending: draft.descending.filter((item) => item !== column),
    });
  };
  return (
    <fieldset className={[styles.keys, styles.wide].join(' ')}>
      <legend>Columns, in key order</legend>
      {columns.map((column) => {
        const position = draft.columns.indexOf(column);
        return (
          <div key={column} className={styles.key}>
            <label className={styles.checkRow}>
              <input
                type="checkbox"
                checked={position >= 0}
                onChange={() => {
                  toggle(column);
                }}
              />
              {column}
              {position >= 0 && ` (${String(position + 1)})`}
            </label>
            {position >= 0 && (
              <select
                aria-label={`Order of ${column}`}
                value={draft.descending.includes(column) ? 'desc' : 'asc'}
                onChange={(event) => {
                  const rest = draft.descending.filter(
                    (item) => item !== column,
                  );
                  onChange({
                    ...draft,
                    descending:
                      event.target.value === 'desc' ? [...rest, column] : rest,
                  });
                }}
              >
                {orders.map((order) => (
                  <option key={order.value} value={order.value}>
                    {order.label}
                  </option>
                ))}
              </select>
            )}
          </div>
        );
      })}
    </fieldset>
  );
}
