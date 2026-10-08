// SPDX-License-Identifier: Apache-2.0
import { SecretField } from './SecretField';
import type { ConnectionForm } from './useConnectionForm';

/** The database password: a [`SecretField`] over the draft's password fields. */
export function PasswordField({
  form,
}: {
  form: ConnectionForm;
}): React.JSX.Element {
  const { draft } = form;
  return (
    <SecretField
      label="Password"
      noun="password"
      secret={{
        value: draft.password,
        mode: draft.passwordMode,
        stored: draft.hasStoredPassword,
      }}
      onChange={(secret) => {
        if (secret.mode === 'clear')
          form.update({ passwordMode: 'clear', password: '' });
        else form.setPassword(secret.value);
      }}
    />
  );
}
