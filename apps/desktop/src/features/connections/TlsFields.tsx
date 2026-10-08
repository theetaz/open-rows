// SPDX-License-Identifier: Apache-2.0
import { Badge, DisclosureSection, SelectField } from '@openrows/ui';
import type { TlsModeDto } from '@openrows/ipc';
import { FileField } from './FileField';
import { SecretField } from './SecretField';
import { tlsWarning } from './routeForm';
import type { ConnectionForm } from './useConnectionForm';
import styles from './ConnectionSheet.module.css';

const modes: readonly { value: TlsModeDto; label: string }[] = [
  { value: 'disable', label: 'Disable' },
  { value: 'allow', label: 'Allow' },
  { value: 'prefer', label: 'Prefer' },
  { value: 'require', label: 'Require (skip verification)' },
  { value: 'verify-ca', label: 'Verify CA' },
  { value: 'verify-full', label: 'Verify full' },
];

const modeLabel = (mode: TlsModeDto): string =>
  modes.find((option) => option.value === mode)?.label ?? mode;

/**
 * "TLS" disclosure: mode, CA certificate and client certificate. A remote
 * connection whose certificate is not verified stays flagged next to the
 * section even when it is collapsed (docs/12 §4.1).
 */
export function TlsFields({
  form,
}: {
  form: ConnectionForm;
}): React.JSX.Element {
  const { draft, errors, update, updateTls } = form;
  const { tls } = draft;
  const warning = tlsWarning(draft.host, draft.tlsMode, tls.caFile);
  return (
    <>
      <DisclosureSection
        label={`TLS · ${modeLabel(draft.tlsMode)}`}
        forceOpen={Boolean(errors.tlsCaFile ?? errors.tlsClientKey)}
      >
        <div className={styles.section}>
          <SelectField
            className={styles.row}
            label="Mode"
            aria-label="TLS mode"
            value={draft.tlsMode}
            options={modes}
            onValueChange={(tlsMode) => {
              update({ tlsMode });
            }}
          />
          <FileField
            label="CA file"
            kind="caCertificate"
            value={tls.caFile}
            placeholder="System trust store"
            error={errors.tlsCaFile}
            onChange={(caFile) => {
              updateTls({ caFile });
            }}
          />
          <FileField
            label="Client cert"
            kind="clientCertificate"
            value={tls.clientCert}
            onChange={(clientCert) => {
              updateTls({ clientCert });
            }}
          />
          <FileField
            label="Client key"
            kind="clientKey"
            value={tls.clientKey}
            error={errors.tlsClientKey}
            onChange={(clientKey) => {
              updateTls({ clientKey });
            }}
          />
          {tls.clientKey.trim() && (
            <SecretField
              label="Passphrase"
              noun="client key passphrase"
              secret={tls.passphrase}
              onChange={(passphrase) => {
                updateTls({ passphrase });
              }}
            />
          )}
        </div>
      </DisclosureSection>
      {warning && (
        <p className={styles.warning} role="note">
          <Badge tone="warning">{warning.badge}</Badge>
          <span>{warning.detail}</span>
        </p>
      )}
    </>
  );
}
