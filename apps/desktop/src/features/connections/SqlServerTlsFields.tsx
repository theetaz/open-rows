// SPDX-License-Identifier: Apache-2.0
import { SelectField } from '@openrows/ui';
import { SwitchRow } from './SwitchRow';
import {
  ENCRYPT_CHOICES,
  encryptOf,
  modeOf,
  trustsCertificate,
} from './sqlServerTls';
import type { ConnectionForm } from './useConnectionForm';
import styles from './ConnectionSheet.module.css';

/**
 * SQL Server's Encrypt and Trust server certificate settings, kept as the
 * profile's TLS mode (docs/12 §4.1).
 */
export function SqlServerTlsFields({
  form,
}: {
  form: ConnectionForm;
}): React.JSX.Element {
  const { draft, update } = form;
  const encrypt = encryptOf(draft.tlsMode);
  return (
    <>
      <SelectField
        className={styles.row}
        label="Encrypt"
        aria-label="Encrypt"
        value={encrypt}
        options={ENCRYPT_CHOICES}
        onValueChange={(choice) => {
          const trust = trustsCertificate(draft.tlsMode);
          update({ tlsMode: modeOf(choice, trust, draft.tlsMode) });
        }}
      />
      {encrypt === 'mandatory' && (
        <SwitchRow
          label="Trust server certificate"
          detail="Encrypt without checking the certificate, e.g. a server's self-signed one."
          checked={trustsCertificate(draft.tlsMode)}
          onChange={(trust) => {
            update({ tlsMode: modeOf('mandatory', trust, draft.tlsMode) });
          }}
        />
      )}
    </>
  );
}
