// SPDX-License-Identifier: Apache-2.0
import {
  DisclosureSection,
  SelectField,
  Switch,
  TextField,
} from '@openrows/ui';
import type { ProxyKindDto } from '@openrows/ipc';
import { fieldError } from './form';
import { SecretField } from './SecretField';
import { PROXY_PORTS, proxySummary } from './proxyForm';
import type { ConnectionForm } from './useConnectionForm';
import styles from './ConnectionSheet.module.css';
import { rich, t } from './messages/routes';

const kinds: readonly { value: ProxyKindDto; label: string }[] = [
  { value: 'socks5', label: t('socks5') },
  { value: 'http', label: t('httpConnect') },
];

/**
 * "Proxy" disclosure: reach the server (or the SSH server) through a SOCKS5
 * or HTTP proxy. Collapsed until the profile uses one or a field is invalid.
 */
export function ProxyFields({
  form,
}: {
  form: ConnectionForm;
}): React.JSX.Element {
  const { proxy } = form.draft;
  const invalid = Boolean(form.errors.proxyHost ?? form.errors.proxyPort);
  return (
    <DisclosureSection
      label={t('proxyProxy', { proxy: proxySummary(proxy) })}
      defaultOpen={proxy.enabled}
      forceOpen={invalid}
    >
      <div className={styles.section}>
        <div className={styles.switchRow}>
          <Switch
            aria-label={t('connectThroughProxy')}
            checked={proxy.enabled}
            onCheckedChange={(enabled) => {
              form.updateProxy({ enabled });
            }}
          />
          <span>
            {rich('connectThroughProxysmall', {
              small: <small>{t('firstHopBefore')}</small>,
            })}
          </span>
        </div>
        {proxy.enabled && <ProxyServer form={form} />}
      </div>
    </DisclosureSection>
  );
}

function ProxyServer({ form }: { form: ConnectionForm }): React.JSX.Element {
  const { proxy } = form.draft;
  const { errors, updateProxy } = form;
  const text = (key: 'host' | 'port' | 'user') => ({
    value: proxy[key],
    spellCheck: false,
    onChange: (event: React.ChangeEvent<HTMLInputElement>) => {
      updateProxy({ [key]: event.target.value });
    },
  });
  return (
    <>
      <SelectField
        className={styles.row}
        label={t('type')}
        aria-label={t('proxyType')}
        value={proxy.kind}
        options={kinds}
        onValueChange={(kind) => {
          updateProxy({ kind });
        }}
      />
      <div className={styles.pair}>
        <TextField
          className={styles.row}
          label={t('proxyHost')}
          placeholder={t('proxyExampleCom')}
          {...text('host')}
          {...fieldError(errors.proxyHost)}
        />
        <TextField
          label={t('port')}
          aria-label={t('proxyPort')}
          inputMode="numeric"
          placeholder={PROXY_PORTS[proxy.kind]}
          className={styles.port}
          {...text('port')}
          {...fieldError(errors.proxyPort)}
        />
      </div>
      <TextField
        className={styles.row}
        label={t('proxyUser')}
        placeholder={t('optional')}
        {...text('user')}
      />
      {proxy.user.trim() && (
        <SecretField
          label={t('proxyPassword')}
          noun={t('proxyPassword2')}
          secret={proxy.password}
          reveal={form.reveal('proxyPassword')}
          onChange={(password) => {
            updateProxy({ password });
          }}
        />
      )}
    </>
  );
}
