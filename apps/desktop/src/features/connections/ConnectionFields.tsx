// SPDX-License-Identifier: Apache-2.0
import { TextField } from '@openrows/ui';
import { fieldError } from './form';
import { PasswordField } from './PasswordField';
import type { ConnectionForm } from './useConnectionForm';
import styles from './ConnectionSheet.module.css';

type TextKey = 'name' | 'group' | 'host' | 'port' | 'user' | 'database';

/** Props of a text field bound to one draft value. */
function binder(form: ConnectionForm) {
  return (key: TextKey) => ({
    className: styles.row,
    value: form.draft[key],
    onChange: (event: React.ChangeEvent<HTMLInputElement>) => {
      form.update({ [key]: event.target.value });
    },
  });
}

/** Name and group, shared by every engine. */
export function NameFields({
  form,
}: {
  form: ConnectionForm;
}): React.JSX.Element {
  const text = binder(form);
  return (
    <>
      <TextField
        label="Name"
        {...text('name')}
        {...fieldError(form.errors.name)}
      />
      <TextField label="Group" placeholder="Optional" {...text('group')} />
    </>
  );
}

/** Name, group, address and credentials, labels leading as in AppKit forms. */
export function ConnectionFields({
  form,
}: {
  form: ConnectionForm;
}): React.JSX.Element {
  const { draft, errors } = form;
  const text = binder(form);
  return (
    <>
      <NameFields form={form} />
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
        placeholder={draft.engine === 'postgres' ? 'postgres' : 'Optional'}
        {...text('database')}
      />
    </>
  );
}
