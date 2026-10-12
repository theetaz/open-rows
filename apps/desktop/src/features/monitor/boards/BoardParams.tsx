// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, SelectField, TextField } from '@openrows/ui';
import type { BoardParam, ParamKind } from '@openrows/ipc';
import styles from './Boards.module.css';

const KINDS = [
  { value: 'text', label: 'Text' },
  { value: 'number', label: 'Number' },
  { value: 'date', label: 'Date' },
  { value: 'enum', label: 'Enum' },
] as const;

/** Inputs for the board's typed parameters; values are bound natively. */
export function ParamInputs(props: {
  params: readonly BoardParam[];
  values: Record<string, string>;
  onChange: (name: string, value: string) => void;
}): React.JSX.Element | null {
  if (!props.params.length) return null;
  return (
    <div className={styles.params} aria-label="Board parameters">
      {props.params.map((param) => {
        const value = props.values[param.name] ?? param.default ?? '';
        const change = (next: string): void => {
          props.onChange(param.name, next);
        };
        if (param.kind === 'enum')
          return (
            <SelectField
              key={param.name}
              label={param.name}
              controlSize="compact"
              value={value}
              options={[
                { value: '', label: '—' },
                ...param.options.map((option) => ({
                  value: option,
                  label: option,
                })),
              ]}
              onValueChange={change}
            />
          );
        return (
          <TextField
            key={param.name}
            label={param.name}
            controlSize="compact"
            type={param.kind === 'date' ? 'date' : 'text'}
            inputMode={param.kind === 'number' ? 'decimal' : undefined}
            value={value}
            onChange={(event) => {
              change(event.target.value);
            }}
          />
        );
      })}
    </div>
  );
}

function ParamRow(props: {
  param: BoardParam;
  onChange: (change: Partial<BoardParam>) => void;
  onRemove: () => void;
}): React.JSX.Element {
  const { param, onChange } = props;
  return (
    <div className={styles.row}>
      <TextField
        label="Name"
        controlSize="compact"
        value={param.name}
        onChange={(event) => {
          onChange({ name: event.target.value });
        }}
      />
      <SelectField
        label="Type"
        controlSize="compact"
        value={param.kind}
        options={KINDS}
        onValueChange={(kind: ParamKind) => {
          onChange({ kind, options: kind === 'enum' ? param.options : [] });
        }}
      />
      <TextField
        label="Default"
        controlSize="compact"
        value={param.default ?? ''}
        onChange={(event) => {
          onChange({ default: event.target.value || null });
        }}
      />
      {param.kind === 'enum' && (
        <TextField
          label="Options (comma-separated)"
          controlSize="compact"
          value={param.options.join(', ')}
          onChange={(event) => {
            const options = event.target.value
              .split(',')
              .map((option) => option.trim())
              .filter(Boolean);
            onChange({ options });
          }}
        />
      )}
      <Button size="sm" variant="ghost" onClick={props.onRemove}>
        Remove
      </Button>
    </div>
  );
}

/** Declare, change and remove typed parameters; saved with the board. */
export function ParamsEditor(props: {
  params: readonly BoardParam[];
  onSave: (params: BoardParam[]) => void;
}): React.JSX.Element {
  const [draft, setDraft] = useState<BoardParam[]>([...props.params]);
  const add = (): void => {
    const name = `p${String(draft.length + 1)}`;
    setDraft([...draft, { name, kind: 'text', default: null, options: [] }]);
  };
  return (
    <section className={styles.editor} aria-label="Edit parameters">
      {draft.map((param, index) => (
        <ParamRow
          key={index}
          param={param}
          onChange={(change) => {
            setDraft(
              draft.map((item, at) =>
                at === index ? { ...item, ...change } : item,
              ),
            );
          }}
          onRemove={() => {
            setDraft(draft.filter((_, at) => at !== index));
          }}
        />
      ))}
      <div className={styles.row}>
        <Button size="sm" onClick={add}>
          Add parameter
        </Button>
        <Button
          size="sm"
          variant="primary"
          onClick={() => {
            props.onSave(draft);
          }}
        >
          Save parameters
        </Button>
      </div>
    </section>
  );
}
