// SPDX-License-Identifier: Apache-2.0
import { SelectField, TextField } from '@openrows/ui';
import type { ProxyKindDto } from '@openrows/ipc';
import { fieldError } from './form';
import { SecretField } from './SecretField';
import { SwitchRow } from './SwitchRow';
import { PROXY_PORTS } from './proxyForm';
import type { ConnectionForm } from './useConnectionForm';
import styles from './flow/Form.module.css';
import { t } from './messages/routes';

const kinds: readonly { value: ProxyKindDto; label: string }[] = [
  { value: 'socks5', label: t('socks5') },
  { value: 'http', label: t('httpConnect') },
];

/** Proxy rows: a switch that reveals a SOCKS5 or HTTP proxy, the first hop. */
export function ProxyFields({
  form,
}: {
  form: ConnectionForm;
}): React.JSX.Element {
  const { proxy } = form.draft;
  return (
    <>
      <SwitchRow
        label={t('connectThroughProxy')}
        detail={t('firstHopBefore')}
        checked={proxy.enabled}
        onChange={(enabled) => {
          form.updateProxy({ enabled });
        }}
      />
      {proxy.enabled && <ProxyServer form={form} />}
    </>
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
      <div className={`${styles.pair ?? ''} ${styles.mono ?? ''}`}>
        <TextField
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
