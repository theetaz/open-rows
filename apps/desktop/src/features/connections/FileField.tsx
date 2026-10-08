// SPDX-License-Identifier: Apache-2.0
import { Button, TextField } from '@openrows/ui';
import { pickConnectionFile } from '@openrows/ipc';
import type { PickFileKind } from '@openrows/ipc';
import styles from './ConnectionSheet.module.css';

/**
 * A key or certificate path: typed, or chosen in the native open panel. Only
 * the path is stored; the core reads the file when connecting.
 */
export function FileField(props: {
  label: string;
  kind: PickFileKind;
  value: string;
  placeholder?: string;
  error?: string | undefined;
  onChange: (path: string) => void;
}): React.JSX.Element {
  return (
    <div className={styles.pair}>
      <TextField
        className={styles.row}
        label={props.label}
        value={props.value}
        placeholder={props.placeholder ?? 'None'}
        spellCheck={false}
        {...(props.error ? { error: props.error } : {})}
        onChange={(event) => {
          props.onChange(event.target.value);
        }}
      />
      <Button
        aria-label={`Choose ${props.label.toLowerCase()}`}
        onClick={() => {
          // A panel that fails to open leaves the typed path; nothing to undo.
          pickConnectionFile(props.kind).then(
            (path) => {
              if (path) props.onChange(path);
            },
            () => undefined,
          );
        }}
      >
        Choose…
      </Button>
    </div>
  );
}
