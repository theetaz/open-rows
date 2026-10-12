// SPDX-License-Identifier: Apache-2.0
import { TextField } from '@openrows/ui';
import type { ConnectionForm } from './useConnectionForm';

/** MongoDB discovery and authentication database, shared with URL fields. */
export function MongoFields({
  form,
}: {
  form: ConnectionForm;
}): React.JSX.Element {
  const value = form.draft.mongodb ?? { srv: false, authSource: '' };
  return (
    <fieldset>
      <legend>MongoDB</legend>
      <label>
        <input
          type="checkbox"
          checked={value.srv}
          onChange={(event) => {
            form.update({
              mongodb: { ...value, srv: event.target.checked },
              tlsMode: event.target.checked
                ? 'verify-full'
                : form.draft.tlsMode,
            });
          }}
        />
        Discover servers with DNS SRV
      </label>
      <TextField
        label="Authentication database"
        placeholder="Database in URL, or admin"
        value={value.authSource}
        onChange={(event) => {
          form.update({
            mongodb: { ...value, authSource: event.target.value },
          });
        }}
      />
    </fieldset>
  );
}
