// SPDX-License-Identifier: Apache-2.0
import { TextField } from '@openrows/ui';
import { NO_MONGO_OPTIONS } from './form';
import { SwitchRow } from './SwitchRow';
import type { ConnectionForm } from './useConnectionForm';
import styles from './ConnectionSheet.module.css';

/** MongoDB discovery and authentication database, shared with URL fields. */
export function MongoFields({
  form,
}: {
  form: ConnectionForm;
}): React.JSX.Element {
  const value = form.draft.mongodb ?? NO_MONGO_OPTIONS;
  return (
    <>
      <SwitchRow
        label="DNS seed list (SRV)"
        detail="Connect with mongodb+srv: discover servers and options from DNS. TLS is verified."
        checked={value.srv}
        onChange={(srv) => {
          form.update({
            mongodb: { ...value, srv, direct: srv ? false : value.direct },
            tlsMode: srv ? 'verify-full' : form.draft.tlsMode,
          });
        }}
      />
      {!value.srv && (
        <SwitchRow
          label="Direct connection"
          detail="Talk only to this server, without replica-set discovery, e.g. one member or a container whose advertised host is unreachable."
          checked={value.direct}
          onChange={(direct) => {
            form.update({ mongodb: { ...value, direct } });
          }}
        />
      )}
      <TextField
        className={styles.row}
        label="Auth database"
        placeholder="Database in URL, or admin"
        value={value.authSource}
        onChange={(event) => {
          form.update({
            mongodb: { ...value, authSource: event.target.value },
          });
        }}
      />
    </>
  );
}
