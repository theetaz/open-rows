// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, TextField } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import styles from './Query.module.css';

/**
 * Values for the run's query parameters, prefilled with the tab's last
 * values. They are sent as bound parameters, never pasted into the SQL.
 */
export function ParamsSheet(props: {
  names: readonly string[];
  last: Readonly<Record<string, string>>;
  onRun: (values: Record<string, string>) => void;
  onCancel: () => void;
}): React.JSX.Element {
  const [values, setValues] = useState<Record<string, string>>(() =>
    Object.fromEntries(
      props.names.map((name) => [name, props.last[name] ?? '']),
    ),
  );
  const run = (): void => {
    props.onRun(values);
  };
  return (
    <Sheet title="Query parameters" size="alert" onClose={props.onCancel}>
      <p>Values are sent to the server as bound parameters.</p>
      <div className={styles.params}>
        {props.names.map((name, index) => (
          <TextField
            key={name}
            label={name}
            controlSize="compact"
            {...(index === 0 ? { 'data-initial-focus': true } : {})}
            value={values[name] ?? ''}
            onChange={(event) => {
              setValues({ ...values, [name]: event.target.value });
            }}
            onKeyDown={(event) => {
              if (event.key === 'Enter' && !event.nativeEvent.isComposing) {
                event.preventDefault();
                run();
              }
            }}
          />
        ))}
      </div>
      <SheetActions>
        <Button onClick={props.onCancel}>Cancel</Button>
        <Button variant="primary" onClick={run}>
          Run
        </Button>
      </SheetActions>
    </Sheet>
  );
}
