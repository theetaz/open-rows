// SPDX-License-Identifier: Apache-2.0
import { TextField } from '@openrows/ui';
import type { RestoreCheckDto } from '@openrows/ipc';
import { CommandList } from './ToolList';
import styles from './Dump.module.css';
import { t } from './messages';

/** The checked restore's commands, warnings and, when asked, the typed name. */
export function RestoreConfirm(props: {
  check: RestoreCheckDto;
  typed: string;
  onTyped: (typed: string) => void;
}): React.JSX.Element {
  const { check } = props;
  if (check.blocked)
    return (
      <p className={styles.error} role="alert">
        {check.blocked}
      </p>
    );
  return (
    <>
      <CommandList commands={check.commands} />
      {check.warnings.map((warning) => (
        <p key={warning} className={styles.warning} role="status">
          {warning}
        </p>
      ))}
      {check.typed && (
        <TextField
          label={t('typeDatabaseConfirm', { database: check.database })}
          spellCheck={false}
          autoComplete="off"
          value={props.typed}
          onChange={(event) => {
            props.onTyped(event.target.value);
          }}
        />
      )}
    </>
  );
}
