// SPDX-License-Identifier: Apache-2.0
import { SecureField, SelectField } from '@openrows/ui';
import type { PasswordStorageDto } from '@openrows/ipc';
import { CloudAuthFields } from './CloudAuthFields';
import { cloudProviders } from './cloudAuthForm';
import { CredentialFields } from './CredentialFields';
import { SecretField } from './SecretField';
import type { ConnectionForm } from './useConnectionForm';
import styles from './flow/Form.module.css';
import { t } from './messages/profile';

const storageChoices: readonly { value: PasswordStorageDto; label: string }[] =
  [
    { value: 'keychain', label: t('saveKeychain') },
    { value: 'ask', label: t('askEveryTime') },
    { value: 'none', label: t('noPassword') },
    { value: 'command', label: t('runCommand') },
  ];
const cloudChoice = { value: 'cloud', label: t('cloudIamToken') } as const;

/**
 * The database password and where it lives (TablePlus): a [`SecretField`]
 * saved in the Keychain, or asked for on every connect and never saved.
 */
export function PasswordField({
  form,
}: {
  form: ConnectionForm;
}): React.JSX.Element {
  const { draft } = form;
  return (
    <>
      {draft.passwordStorage === 'cloud' ? (
        <CloudAuthFields form={form} />
      ) : draft.passwordStorage === 'command' ? (
        <CredentialFields form={form} />
      ) : draft.passwordStorage === 'none' ? (
        <small className={styles.hint}>{t('noDatabasePassword')}</small>
      ) : draft.passwordStorage === 'ask' ? (
        <AskedPassword form={form} />
      ) : (
        <SecretField
          label={draft.engine === 'libsql' ? t('authToken') : t('password')}
          noun={draft.engine === 'libsql' ? t('authToken2') : t('password2')}
          secret={{
            value: draft.password,
            mode: draft.passwordMode,
            stored: draft.hasStoredPassword,
          }}
          reveal={form.reveal('password')}
          onChange={(secret) => {
            if (secret.mode === 'clear')
              form.update({ passwordMode: 'clear', password: '' });
            else form.setPassword(secret.value);
          }}
        />
      )}
      <SelectField
        className={styles.row}
        label={t('store')}
        aria-label={
          draft.engine === 'libsql'
            ? t('authTokenStorage')
            : t('passwordStorage')
        }
        value={draft.passwordStorage}
        options={
          cloudProviders(draft.engine).length > 0
            ? [...storageChoices, cloudChoice]
            : storageChoices
        }
        onValueChange={(passwordStorage) => {
          form.update({ passwordStorage });
        }}
      />
    </>
  );
}

/** "Ask every time": a password typed here is only used by Test. */
function AskedPassword({ form }: { form: ConnectionForm }): React.JSX.Element {
  const { draft } = form;
  return (
    <div className={styles.stack}>
      <SecureField
        className={styles.row}
        label={draft.engine === 'libsql' ? t('authToken') : t('password')}
        placeholder={t('askedWhenConnecting')}
        value={draft.password}
        onChange={(event) => {
          form.setPassword(event.target.value);
        }}
      />
      <small className={styles.hint}>
        {draft.hasStoredPassword
          ? t('savedPasswordRemoved')
          : t('neverSavedYou')}
      </small>
    </div>
  );
}
