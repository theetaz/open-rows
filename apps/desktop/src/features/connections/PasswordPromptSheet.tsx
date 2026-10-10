// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, SecureField } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import type { PasswordRequest } from './usePasswordPrompt';
import styles from './ConnectionSheet.module.css';

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
  const credential =
    request.profile.engine === 'libsql' ? 'auth token' : 'password';
  return (
    <Sheet
      title={`Connect to ${request.profile.name}`}
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
        <p>{`Enter the ${credential} for ${address(request)}.`}</p>
        <SecureField
          className={styles.row}
          label={
            request.profile.engine === 'libsql' ? 'Auth token' : 'Password'
          }
          autoFocus
          value={password}
          onChange={(event) => {
            setPassword(event.target.value);
          }}
        />
        <small className={styles.note}>
          Used for this connection only and never saved.
        </small>
        <SheetActions>
          <Button onClick={request.cancel}>Cancel</Button>
          <Button type="submit" variant="primary">
            Connect
          </Button>
        </SheetActions>
      </form>
    </Sheet>
  );
}
