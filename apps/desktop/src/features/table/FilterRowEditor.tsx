// SPDX-License-Identifier: Apache-2.0
import type { KeyboardEvent } from 'react';
import type { FilterOperatorDto } from '@openrows/ipc';
import { Button, IconButton, SelectField, TextField } from '@openrows/ui';
import { arity, operators } from './model';
import type { FilterRow } from './model';
import type { TableActions } from './useTableTab';
import styles from './Table.module.css';

type Props = {
  row: FilterRow;
  index: number;
  columns: readonly string[];
  actions: TableActions;
};

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

/** [column ▾] [operator ▾] */
function Target({ row, columns, label, edit }: Part): React.JSX.Element {
  return (
    <>
      <SelectField
        aria-label={`Column of ${label}`}
        className={styles.picker}
        controlSize="compact"
        value={row.column}
        options={columns.map((column) => ({ value: column, label: column }))}
        onValueChange={(column) => {
          edit({ column });
        }}
      />
      <SelectField<FilterOperatorDto>
        aria-label={`Operator of ${label}`}
        className={styles.picker}
        controlSize="compact"
        value={row.operator}
        options={operators}
        onValueChange={(operator) => {
          edit({ operator });
        }}
      />
    </>
  );
}

/** [value] and, for BETWEEN, [upper bound]; NULL checks take no value. */
function Values({ row, actions, label, edit }: Part): React.JSX.Element {
  const fields = arity(row.operator);
  return (
    <>
      <TextField
        className={styles.value}
        controlSize="compact"
        aria-label={`Value of ${label}`}
        placeholder={
          row.operator === 'in' || row.operator === 'notIn'
            ? 'a, b, c'
            : 'Value'
        }
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
