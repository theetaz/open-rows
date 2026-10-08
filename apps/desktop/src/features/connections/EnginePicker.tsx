// SPDX-License-Identifier: Apache-2.0
import { useId } from 'react';
import { Button, Icon, TextField } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import type { EngineDto } from '@openrows/ipc';
import { EngineTile } from './EngineTile';
import type { EngineChoice } from './engines';
import { useEnginePicker } from './useEnginePicker';
import type { EnginePicker as PickerState } from './useEnginePicker';
import styles from './EnginePicker.module.css';

/**
 * "New connection", step one (TablePlus): search and a grid of neutral
 * engine tiles. Arrows move the outlined tile, Return or a double-click
 * creates, Escape cancels. Types without a driver say "Coming soon".
 */
export function EnginePicker(props: {
  onCancel: () => void;
  onCreate: (engine: EngineDto) => void;
}): React.JSX.Element {
  const picker = useEnginePicker(props.onCreate);
  return (
    <Sheet title="New connection" onClose={props.onCancel}>
      <TextField
        aria-label="Search connection types"
        type="search"
        placeholder="Search"
        autoFocus
        autoComplete="off"
        spellCheck={false}
        leadingIcon={<Icon name="search" />}
        value={picker.query}
        onChange={(event) => {
          picker.setQuery(event.target.value);
        }}
        onKeyDown={(event) => {
          const vertical = event.key === 'ArrowUp' || event.key === 'ArrowDown';
          if (vertical && picker.move(event.key)) event.preventDefault();
        }}
      />
      <EngineGrid picker={picker} />
      <p role="status" className={styles.status}>
        {statusText(picker.current)}
      </p>
      <SheetActions>
        <Button onClick={props.onCancel}>Cancel</Button>
        <Button
          variant="primary"
          disabled={!picker.current?.engine}
          onClick={picker.create}
        >
          Create
        </Button>
      </SheetActions>
    </Sheet>
  );
}

function statusText(choice: EngineChoice | undefined): string {
  if (!choice) return '';
  return choice.engine ? '' : `${choice.name} connections are coming soon.`;
}

function EngineGrid({ picker }: { picker: PickerState }): React.JSX.Element {
  const id = useId();
  const optionId = (choice: EngineChoice): string => `${id}-${choice.id}`;
  if (!picker.visible.length)
    return <p className={styles.empty}>No connection type matches.</p>;
  return (
    <div
      role="listbox"
      aria-label="Connection types"
      tabIndex={0}
      className={styles.grid}
      aria-activedescendant={picker.current && optionId(picker.current)}
      onKeyDown={(event) => {
        if (picker.move(event.key)) event.preventDefault();
      }}
    >
      {picker.visible.map((choice) => (
        <div
          key={choice.id}
          id={optionId(choice)}
          role="option"
          aria-selected={choice.id === picker.current?.id}
          aria-disabled={!choice.engine}
          className={styles.option}
          onClick={() => {
            picker.select(choice.id);
          }}
          onDoubleClick={() => {
            picker.select(choice.id);
            picker.create();
          }}
        >
          <EngineTile monogram={choice.monogram} />
          <span className={styles.name}>{choice.name}</span>
        </div>
      ))}
    </div>
  );
}
