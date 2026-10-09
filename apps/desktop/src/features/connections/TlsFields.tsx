// SPDX-License-Identifier: Apache-2.0
import { Badge, DisclosureSection, SelectField } from '@openrows/ui';
import type { CloudCaDto, TlsModeDto } from '@openrows/ipc';
import { FileField } from './FileField';
import { SecretField } from './SecretField';
import { tlsCa, tlsWarning } from './routeForm';
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

type CloudChoice = CloudCaDto | 'none';

const cloudChoices: readonly { value: CloudChoice; label: string }[] = [
  { value: 'none', label: 'None' },
  { value: 'awsRds', label: 'AWS RDS / Aurora' },
  { value: 'azure', label: 'Azure Database' },
];

const modeLabel = (mode: TlsModeDto): string =>
  modes.find((option) => option.value === mode)?.label ?? mode;

/**
 * "TLS" disclosure: mode, a bundled cloud provider CA or a CA file, and
 * the client certificate. A remote
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
  const warning = tlsWarning(draft.host, draft.tlsMode, tlsCa(tls));
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
          <CaFields form={form} />
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
              reveal={form.reveal('tlsKeyPassphrase')}
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

/** Trust anchor: a bundled cloud provider CA, or a CA file (else the system store). */
function CaFields({ form }: { form: ConnectionForm }): React.JSX.Element {
  const { errors, updateTls } = form;
  const { tls } = form.draft;
  return (
    <>
      <SelectField
        className={styles.row}
        label="Cloud CA"
        aria-label="Cloud provider CA"
        value={tls.cloudCa ?? 'none'}
        options={cloudChoices}
        onValueChange={(choice) => {
          updateTls({ cloudCa: choice === 'none' ? null : choice });
        }}
      />
      {!tls.cloudCa && (
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
      )}
    </>
  );
}
