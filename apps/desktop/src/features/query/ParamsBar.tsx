// SPDX-License-Identifier: Apache-2.0
import { useEffect, useId, useRef, useState } from 'react';
import { Button, Icon, SelectField, TextField } from '@openrows/ui';
import { isValidParam } from './paramValues';
import type { ParamKind, ParamValue, ParamValues } from './paramValues';
import styles from './ParamsBar.module.css';

type Props = {
  /** Parameter names of the tab's SQL, in order of first use. */
  names: readonly string[];
  values: ParamValues;
  /** The last run waits for values (the core asked for them). */
  asking: boolean;
  onChange: (values: ParamValues) => void;
  /** Run again with the values (Enter in a field, or Run). */
  onRun: () => void;
};

const KINDS: readonly { value: ParamKind; label: string }[] = [
  { value: 'text', label: 'Text' },
  { value: 'number', label: 'Number' },
  { value: 'null', label: 'NULL' },
];

const EMPTY: ParamValue = { kind: 'text', value: '' };

/** Focus the first field without a value once the core asks for values. */
function useFocusMissing(
  asking: boolean,
  list: React.RefObject<HTMLDivElement | null>,
): void {
  useEffect(() => {
    if (!asking) return;
    list.current
      ?.querySelector<HTMLInputElement>('input[data-missing="true"], input')
      ?.focus();
  }, [asking, list]);
}

/**
 * The collapsible bar between the editor and its results listing the SQL's
 * parameters with typed inputs. Values stay on the tab and every run binds
 * them; they are never pasted into the SQL.
 */
export function ParamsBar(props: Props): React.JSX.Element {
  const [open, setOpen] = useState(true);
  const list = useRef<HTMLDivElement>(null);
  const id = useId();
  const expanded = open || props.asking;
  useFocusMissing(props.asking, list);
  const set = (name: string, param: ParamValue): void => {
    props.onChange({ ...props.values, [name]: param });
  };
  return (
    <section className={styles.bar} aria-label="Query parameters">
      <header className={styles.header}>
        <button
          type="button"
          className={styles.toggle}
          aria-expanded={expanded}
          aria-controls={id}
          onClick={() => {
            setOpen(!expanded);
          }}
        >
          <span className={styles.chevron} data-open={expanded}>
            <Icon name="chevronRight" />
          </span>
          Parameters
          <span className={styles.count}>{props.names.length}</span>
        </button>
        {props.asking && (
          <span className={styles.hint} role="status">
            Enter the parameter values, then run.
          </span>
        )}
        <Button size="sm" variant="ghost" onClick={props.onRun}>
          Run
        </Button>
      </header>
      {expanded && (
        <div id={id} ref={list} className={styles.list}>
          {props.names.map((name) => (
            <ParamRow
              key={name}
              name={name}
              param={props.values[name]}
              onChange={(param) => {
                set(name, param);
              }}
              onRun={props.onRun}
            />
          ))}
        </div>
      )}
    </section>
  );
}

function ParamRow(props: {
  name: string;
  param: ParamValue | undefined;
  onChange: (param: ParamValue) => void;
  onRun: () => void;
}): React.JSX.Element {
  const param = props.param ?? EMPTY;
  const isNull = param.kind === 'null';
  return (
    <div className={styles.row}>
      <span className={styles.name}>{props.name}</span>
      <SelectField
        aria-label={`${props.name} type`}
        controlSize="compact"
        className={styles.kind}
        value={param.kind}
        options={KINDS}
        onValueChange={(kind) => {
          props.onChange({ ...param, kind });
        }}
      />
      <TextField
        aria-label={`${props.name} value`}
        controlSize="compact"
        className={styles.value}
        data-missing={props.param === undefined}
        {...(isValidParam(param) ? {} : { error: 'Not a number' })}
        inputMode={param.kind === 'number' ? 'decimal' : 'text'}
        placeholder={isNull ? 'NULL' : ''}
        disabled={isNull}
        value={isNull ? '' : param.value}
        onChange={(event) => {
          props.onChange({ ...param, value: event.target.value });
        }}
        onKeyDown={(event) => {
          if (event.key !== 'Enter' || event.nativeEvent.isComposing) return;
          event.preventDefault();
          props.onRun();
        }}
      />
    </div>
  );
}
