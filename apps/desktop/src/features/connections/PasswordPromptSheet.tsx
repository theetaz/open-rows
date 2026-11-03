// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, SecureField } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import type { PasswordRequest } from './usePasswordPrompt';
import styles from './ConnectionSheet.module.css';
import { t } from './messages/profile';

/** Where the connection logs in, e.g. `app@db.internal:5432`. */
function address(request: PasswordRequest): string {
  const { profile } = request;
  const server = `${profile.host}:${String(profile.port)}`;
  return profile.user ? `${profile.user}@${server}` : server;
}

/**
 * The password of an "ask every time" connection, asked on connect. It is
 * held by the core only while the connection is open and never saved.
 */
export function PasswordPromptSheet({
  request,
}: {
  request: PasswordRequest;
}): React.JSX.Element {
  const [password, setPassword] = useState('');
  return (
    <Sheet
      title={t('connectName', { name: request.profile.name })}
      size="alert"
      onClose={request.cancel}
    >
      <form
        className={styles.form}
        noValidate
        onSubmit={(event) => {
          event.preventDefault();
          request.submit(password);
        }}
      >
        <p>
          {request.profile.engine === 'libsql'
            ? t('enterAuthToken', { request: address(request) })
            : t('enterPassword', { request: address(request) })}
        </p>
        <SecureField
          className={styles.row}
          label={
            request.profile.engine === 'libsql' ? t('authToken') : t('password')
          }
          autoFocus
          value={password}
          onChange={(event) => {
            setPassword(event.target.value);
          }}
        />
        <small className={styles.note}>{t('usedConnectionOnly')}</small>
        <SheetActions>
          <Button onClick={request.cancel}>{t('cancel')}</Button>
          <Button type="submit" variant="primary">
            {t('connect')}
          </Button>
        </SheetActions>
      </form>
    </Sheet>
  );
}
