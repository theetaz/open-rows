// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, SelectField, TextField } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import type { IndexMethodDto, SchemaChangeDto } from '@openrows/ipc';
import styles from './Structure.module.css';

const methods: readonly { value: IndexMethodDto; label: string }[] = [
  { value: 'btree', label: 'B-tree' },
  { value: 'hash', label: 'Hash' },
  { value: 'gin', label: 'GIN' },
  { value: 'gist', label: 'GiST' },
  { value: 'brin', label: 'BRIN' },
  { value: 'spgist', label: 'SP-GiST' },
];

/** `+ Index`: name, key columns (in the order chosen), unique and method. */
export function IndexSheet({
  columns,
  onStage,
  onClose,
}: {
  /** Column names after the staged changes. */
  columns: readonly string[];
  onStage: (change: SchemaChangeDto) => void;
  onClose: () => void;
}): React.JSX.Element {
  const [name, setName] = useState('');
  const [chosen, setChosen] = useState<readonly string[]>([]);
  const [unique, setUnique] = useState(false);
  const [method, setMethod] = useState<IndexMethodDto>('btree');
  return (
    <Sheet title="Add index" onClose={onClose}>
      <div className={styles.form}>
        <TextField
          label="Name"
          value={name}
          placeholder="Automatic"
          onChange={(event) => {
            setName(event.target.value);
          }}
        />
        <KeyColumns columns={columns} chosen={chosen} onChosen={setChosen} />
        <SelectField
          label="Algorithm"
          value={method}
          options={methods}
          onValueChange={setMethod}
        />
        <label className={styles.checkRow}>
          <input
            type="checkbox"
            checked={unique}
            onChange={(event) => {
              setUnique(event.target.checked);
            }}
          />
          Unique
        </label>
        <p className={styles.formNote}>
          The index is built when you commit; building blocks writes to the
          table until it finishes.
        </p>
      </div>
      <SheetActions>
        <Button onClick={onClose}>Cancel</Button>
        <Button
          variant="primary"
          disabled={!chosen.length}
          onClick={() => {
            onStage({
              kind: 'createIndex',
              name: name.trim() || null,
              columns: [...chosen],
              unique,
              method,
            });
            onClose();
          }}
        >
          Stage index
        </Button>
      </SheetActions>
    </Sheet>
  );
}

/** Key columns as checkboxes; the click order is the key order. */
function KeyColumns({
  columns,
  chosen,
  onChosen,
}: {
  columns: readonly string[];
  chosen: readonly string[];
  onChosen: (chosen: readonly string[]) => void;
}): React.JSX.Element {
  return (
    <fieldset className={styles.columnsPick}>
      <legend>Columns, in key order</legend>
      {columns.map((column) => (
        <label key={column}>
          <input
            type="checkbox"
            checked={chosen.includes(column)}
            onChange={() => {
              onChosen(
                chosen.includes(column)
                  ? chosen.filter((item) => item !== column)
                  : [...chosen, column],
              );
            }}
          />
          {column}
          {chosen.includes(column) &&
            ` (${String(chosen.indexOf(column) + 1)})`}
        </label>
      ))}
    </fieldset>
  );
}
