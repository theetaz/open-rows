// SPDX-License-Identifier: Apache-2.0
import { Badge, DisclosureSection, SelectField } from '@openrows/ui';
import type { CloudCaDto, TlsModeDto } from '@openrows/ipc';
import { FileField } from './FileField';
import { SqlServerTlsFields } from './SqlServerTlsFields';
import { encryptLabel } from './sqlServerTls';
import { SecretField } from './SecretField';
import { tlsCa, tlsWarning } from './routeForm';
import type { ConnectionForm } from './useConnectionForm';
import type { Draft } from './form';
import styles from './ConnectionSheet.module.css';
import { t } from './messages/routes';

const modes: readonly { value: TlsModeDto; label: string }[] = [
  { value: 'disable', label: t('disable') },
  { value: 'allow', label: t('allow') },
  { value: 'prefer', label: t('prefer') },
  { value: 'require', label: t('requireSkipVerification') },
  { value: 'verify-ca', label: t('verifyCa') },
  { value: 'verify-full', label: t('verifyFull') },
];

type CloudChoice = CloudCaDto | 'none';

const cloudChoices: readonly { value: CloudChoice; label: string }[] = [
  { value: 'none', label: t('none') },
  { value: 'awsRds', label: t('awsRdsAurora') },
  { value: 'azure', label: t('azureDatabase') },
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
  const sqlServer = draft.engine === 'sqlserver';
  const title = sqlServer
    ? t('encryptTlsmode', { tlsMode: encryptLabel(draft.tlsMode) })
    : t('tlsTitle', { mode: modeLabel(draft.tlsMode) });
  return (
    <>
      <DisclosureSection
        label={title}
        forceOpen={Boolean(errors.tlsCaFile ?? errors.tlsClientKey)}
      >
        <div className={styles.section}>
          {sqlServer ? (
            <SqlServerTlsFields form={form} />
          ) : (
            <SelectField
              className={styles.row}
              label={t('mode')}
              aria-label={t('tlsMode')}
              value={draft.tlsMode}
              options={
                draft.engine === 'clickhouse'
                  ? modes.filter(
                      ({ value }) => !['allow', 'prefer'].includes(value),
                    )
                  : modes
              }
              onValueChange={(tlsMode) => {
                update(tlsUpdate(draft, tlsMode));
              }}
            />
          )}
          <CaFields form={form} />
          <FileField
            label={t('clientCert')}
            kind="clientCertificate"
            value={tls.clientCert}
            onChange={(clientCert) => {
              updateTls({ clientCert });
            }}
          />
          <FileField
            label={t('clientKey')}
            kind="clientKey"
            value={tls.clientKey}
            error={errors.tlsClientKey}
            onChange={(clientKey) => {
              updateTls({ clientKey });
            }}
          />
          {tls.clientKey.trim() && (
            <SecretField
              label={t('passphrase')}
              noun={t('clientKeyPassphrase')}
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
        label={t('cloudCa')}
        aria-label={t('cloudProviderCa')}
        value={tls.cloudCa ?? 'none'}
        options={cloudChoices}
        onValueChange={(choice) => {
          updateTls({ cloudCa: choice === 'none' ? null : choice });
        }}
      />
      {!tls.cloudCa && (
        <FileField
          label={t('caFile')}
          kind="caCertificate"
          value={tls.caFile}
          placeholder={t('systemTrustStore')}
          error={errors.tlsCaFile}
          onChange={(caFile) => {
            updateTls({ caFile });
          }}
        />
      )}
    </>
  );
}

function tlsUpdate(draft: Draft, tlsMode: TlsModeDto): Partial<Draft> {
  const https = ['require', 'verify-ca', 'verify-full'].includes(tlsMode);
  const defaultHttp = ['8123', '8443'].includes(draft.port);
  return {
    tlsMode,
    ...(draft.engine === 'clickhouse' && defaultHttp
      ? { port: https ? '8443' : '8123' }
      : {}),
  };
}
