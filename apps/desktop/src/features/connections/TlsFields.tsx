// SPDX-License-Identifier: Apache-2.0
import { Badge, SelectField } from '@openrows/ui';
import type { CloudCaDto, TlsModeDto } from '@openrows/ipc';
import { FileField } from './FileField';
import { SqlServerTlsFields } from './SqlServerTlsFields';
import { SecretField } from './SecretField';
import { tlsCa, tlsWarning } from './routeForm';
import type { ConnectionForm } from './useConnectionForm';
import type { Draft } from './form';
import styles from './flow/Form.module.css';
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

const hints: Record<TlsModeDto, string> = {
  disable: t('tlsHintDisable'),
  allow: t('tlsHintAllow'),
  prefer: t('tlsHintPrefer'),
  require: t('tlsHintRequire'),
  'verify-ca': t('tlsHintVerifyCa'),
  'verify-full': t('tlsHintVerifyFull'),
};

/** TLS mode label, e.g. `Verify full`. */
export function tlsModeLabel(mode: TlsModeDto): string {
  return modes.find((option) => option.value === mode)?.label ?? mode;
}

/**
 * Encryption rows: the mode with its consequence (SQL Server's Encrypt and
 * Trust instead), a bundled cloud CA or a CA file, and the client
 * certificate. Unverified TLS to a remote host stays flagged (docs/12 §4.1).
 */
export function TlsFields({
  form,
}: {
  form: ConnectionForm;
}): React.JSX.Element {
  const { draft, errors, updateTls } = form;
  const { tls } = draft;
  const warning = tlsWarning(draft.host, draft.tlsMode, tlsCa(tls));
  return (
    <>
      {draft.engine === 'sqlserver' ? (
        <SqlServerTlsFields form={form} />
      ) : (
        <TlsModeRow form={form} />
      )}
      {warning && (
        <p className={styles.hint} role="note">
          <Badge tone="warning">{warning.badge}</Badge>
          <span>{warning.detail}</span>
        </p>
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
    </>
  );
}

/** The mode pop-up with a one-line consequence; ClickHouse has no allow or prefer. */
function TlsModeRow({ form }: { form: ConnectionForm }): React.JSX.Element {
  const { draft, update } = form;
  return (
    <div className={styles.choiceRow}>
      <span className={styles.stack}>
        <span>{t('tlsMode')}</span>
        <small>{hints[draft.tlsMode]}</small>
      </span>
      <SelectField
        aria-label={t('tlsMode')}
        value={draft.tlsMode}
        options={
          draft.engine === 'clickhouse'
            ? modes.filter(({ value }) => !['allow', 'prefer'].includes(value))
            : modes
        }
        onValueChange={(tlsMode) => {
          update(tlsUpdate(draft, tlsMode));
        }}
      />
    </div>
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
