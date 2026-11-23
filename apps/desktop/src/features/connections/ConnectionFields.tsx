// SPDX-License-Identifier: Apache-2.0
import { TextField } from '@openrows/ui';
import { fieldError, NO_MONGO_OPTIONS } from './form';
import { MongoAuthField, MongoFields } from './MongoFields';
import { PasswordField } from './PasswordField';
import type { ConnectionForm } from './useConnectionForm';
import { derivedName } from './flow/flowDraft';
import styles from './flow/Form.module.css';
import { t } from './messages/profile';

type TextKey =
  'name' | 'group' | 'host' | 'port' | 'user' | 'database' | 'tags';

/** Props of a text field bound to one draft value. */
function binder(form: ConnectionForm) {
  return (key: TextKey) => ({
    value: form.draft[key],
    spellCheck: false,
    autoComplete: 'off',
    onChange: (event: React.ChangeEvent<HTMLInputElement>) => {
      form.update({ [key]: event.target.value });
    },
  });
}

const mono = `${styles.row ?? ''} ${styles.mono ?? ''}`;

/** Identity rows: the name (derived placeholder), group and tags. */
export function NameFields({
  form,
}: {
  form: ConnectionForm;
}): React.JSX.Element {
  const text = binder(form);
  return (
    <>
      <TextField
        className={styles.row}
        label={t('name')}
        placeholder={derivedName(form.draft)}
        {...text('name')}
        {...fieldError(form.errors.name)}
      />
      <TextField
        className={styles.row}
        label={t('group')}
        placeholder={t('optional')}
        {...text('group')}
      />
      <TextField
        className={styles.row}
        label={t('tags')}
        placeholder={t('commaSeparated')}
        {...text('tags')}
      />
    </>
  );
}

/** Server rows: host and port on one row, the database, MongoDB discovery. */
export function ServerFields({
  form,
}: {
  form: ConnectionForm;
}): React.JSX.Element {
  const { draft, errors } = form;
  const text = binder(form);
  const srv = (draft.mongodb ?? NO_MONGO_OPTIONS).srv;
  return (
    <>
      <div className={`${styles.pair ?? ''} ${styles.mono ?? ''}`}>
        <TextField
          label={t('host')}
          {...text('host')}
          {...fieldError(errors.host)}
        />
        <TextField
          label={t('port')}
          disabled={draft.engine === 'mongodb' && srv}
          inputMode="numeric"
          {...text('port')}
          {...fieldError(errors.port)}
        />
      </div>
      {draft.engine !== 'libsql' && (
        <TextField
          className={mono}
          label={t('database')}
          placeholder={
            draft.engine === 'postgres' ? t('postgres') : t('optional')
          }
          {...text('database')}
        />
      )}
      {draft.engine === 'mongodb' && <MongoFields form={form} />}
    </>
  );
}

/** Sign-in rows: the user, the password or token and where it lives. */
export function SignInFields({
  form,
}: {
  form: ConnectionForm;
}): React.JSX.Element {
  const { draft, errors } = form;
  return (
    <>
      {draft.engine !== 'libsql' && (
        <TextField
          className={mono}
          label={t('user')}
          {...binder(form)('user')}
          {...fieldError(errors.user)}
        />
      )}
      <PasswordField form={form} />
      {draft.engine === 'mongodb' && <MongoAuthField form={form} />}
    </>
  );
}
