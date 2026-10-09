// SPDX-License-Identifier: Apache-2.0
import type { KeyboardEvent } from 'react';
import type { EngineDto, FilterOperatorDto } from '@openrows/ipc';
import { Button, IconButton, SelectField, TextField } from '@openrows/ui';
import { arity, operators } from './model';
import type { OperatorInfo } from './model';
import { operatorsFor } from './filterOperators';
import type { FilterRow } from './model';
import type { TableActions } from './useTableTab';
import styles from './Table.module.css';
import extras from './TableExtras.module.css';

type Props = {
  row: FilterRow;
  index: number;
  columns: readonly string[];
  /** Database type of each column, for the operators it offers. */
  types: ReadonlyMap<string, string>;
  engine: EngineDto;
  actions: TableActions;
};

/** The column picker's entry for a raw SQL predicate (no column). */
const RAW_COLUMN = '';

/** Choosing a column keeps the operator when the column's type allows it. */
export function columnPatch(
  row: FilterRow,
  column: string,
  allowed: readonly { value: FilterOperatorDto }[],
): Partial<FilterRow> {
  if (column === RAW_COLUMN) return { column, operator: 'raw' };
  const keep = allowed.some((item) => item.value === row.operator);
  return { column, operator: keep ? row.operator : 'equal' };
}

type Part = Props & {
  label: string;
  edit: (patch: Partial<FilterRow>) => void;
};

/** Return applies every enabled filter. */
function applyOnReturn(actions: TableActions) {
  return (event: KeyboardEvent<HTMLInputElement>): void => {
    if (event.key !== 'Enter' || event.nativeEvent.isComposing) return;
    event.preventDefault();
    actions.apply();
  };
}

/** [column ▾] [operator ▾]; the column list ends with Raw SQL. */
function Target({
  row,
  columns,
  types,
  engine,
  label,
  edit,
}: Part): React.JSX.Element {
  const choices = (column: string): ReturnType<typeof operatorsFor> =>
    operatorsFor(types.get(column) ?? null, engine);
  const raw = row.operator === 'raw';
  return (
    <>
      <SelectField
        aria-label={`Column of ${label}`}
        className={styles.picker}
        controlSize="compact"
        value={raw ? RAW_COLUMN : row.column}
        options={[
          ...columns.map((column) => ({ value: column, label: column })),
          { value: RAW_COLUMN, label: 'Raw SQL' },
        ]}
        onValueChange={(column) => {
          edit(columnPatch(row, column, choices(column)));
        }}
      />
      {!raw && (
        <SelectField<FilterOperatorDto>
          aria-label={`Operator of ${label}`}
          className={styles.picker}
          controlSize="compact"
          value={row.operator}
          options={withCurrent(choices(row.column), row.operator)}
          onValueChange={(operator) => {
            edit({ operator });
          }}
        />
      )}
    </>
  );
}

/** The column's operators, plus a restored one its type no longer offers. */
function withCurrent(
  choices: OperatorInfo[],
  current: FilterOperatorDto,
): OperatorInfo[] {
  const known = operators.find((item) => item.value === current);
  return choices.some((item) => item.value === current) || !known
    ? choices
    : [...choices, known];
}

/** Placeholder of the value field for an operator. */
function hint(operator: FilterOperatorDto): string {
  if (operator === 'raw') return "e.g. total > 10 AND status <> 'void'";
  if (operator === 'in' || operator === 'notIn') return 'a, b, c';
  return 'Value';
}

/** [value] and, for BETWEEN, [upper bound]; NULL checks take no value. */
function Values({ row, actions, label, edit }: Part): React.JSX.Element {
  const fields = arity(row.operator);
  return (
    <>
      <TextField
        className={row.operator === 'raw' ? extras.rawValue : styles.value}
        controlSize="compact"
        aria-label={
          row.operator === 'raw' ? `SQL of ${label}` : `Value of ${label}`
        }
        placeholder={hint(row.operator)}
        disabled={fields === 0}
        value={fields === 0 ? '' : row.value}
        onChange={(event) => {
          edit({ value: event.target.value });
        }}
        onKeyDown={applyOnReturn(actions)}
      />
      {fields === 2 && (
        <TextField
          className={styles.value}
          controlSize="compact"
          aria-label={`Upper bound of ${label}`}
          value={row.second}
          onChange={(event) => {
            edit({ second: event.target.value });
          }}
          onKeyDown={applyOnReturn(actions)}
        />
      )}
    </>
  );
}

/** [enabled] [column ▾] [operator ▾] [value] [Apply] [−] [+] */
export function FilterRowEditor(props: Props): React.JSX.Element {
  const { row, index, actions } = props;
  const label = `filter ${String(index + 1)}`;
  const part: Part = {
    ...props,
    label,
    edit: (patch) => {
      actions.editRow(row.id, patch);
    },
  };
  return (
    <div
      className={styles.filterRow}
      role="group"
      aria-label={`Filter ${String(index + 1)}`}
    >
      <input
        type="checkbox"
        className={styles.check}
        aria-label={`Enable ${label}`}
        checked={row.enabled}
        onChange={(event) => {
          part.edit({ enabled: event.target.checked });
        }}
      />
      <Target {...part} />
      <Values {...part} />
      <Button
        size="sm"
        aria-label={`Apply ${label}`}
        onClick={() => {
          actions.apply(row.id);
        }}
      >
        Apply
      </Button>
      <IconButton
        size="sm"
        variant="ghost"
        aria-label={`Remove ${label}`}
        onClick={() => {
          actions.removeRow(row.id);
        }}
      >
        −
      </IconButton>
      <IconButton
        size="sm"
        variant="ghost"
        aria-label={`Add filter after ${label}`}
        onClick={() => {
          actions.addRow(row.id);
        }}
      >
        +
      </IconButton>
    </div>
  );
}
