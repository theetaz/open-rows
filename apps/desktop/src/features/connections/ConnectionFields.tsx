// SPDX-License-Identifier: Apache-2.0
import { TextField } from '@openrows/ui';
import { fieldError } from './form';
import { PasswordField } from './PasswordField';
import type { ConnectionForm } from './useConnectionForm';
import styles from './ConnectionSheet.module.css';

/** Name, group, address and credentials, labels leading as in AppKit forms. */
export function ConnectionFields({
  form,
}: {
  form: ConnectionForm;
}): React.JSX.Element {
  const { draft, errors, update } = form;
  const text = (
    key: 'name' | 'group' | 'host' | 'port' | 'user' | 'database',
  ) => ({
    className: styles.row,
    value: draft[key],
    onChange: (event: React.ChangeEvent<HTMLInputElement>) => {
      update({ [key]: event.target.value });
    },
  });
  return (
    <>
      <TextField label="Name" {...text('name')} {...fieldError(errors.name)} />
      <TextField label="Group" placeholder="Optional" {...text('group')} />
      <div className={styles.pair}>
        <TextField
          label="Host"
          {...text('host')}
          {...fieldError(errors.host)}
        />
        <TextField
          label="Port"
          inputMode="numeric"
          {...text('port')}
          className={styles.port}
          {...fieldError(errors.port)}
        />
      </div>
      <TextField label="User" {...text('user')} {...fieldError(errors.user)} />
      <PasswordField form={form} />
      <TextField
        label="Database"
        placeholder="Same as user"
        {...text('database')}
      />
    </>
  );
}
