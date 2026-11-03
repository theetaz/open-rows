// SPDX-License-Identifier: Apache-2.0
import { SelectField, TextField } from '@openrows/ui';
import type { ProviderConfig } from '@openrows/ipc';
import { isCliProvider, PROVIDER_KINDS } from './aiSettings';
import styles from './Ai.module.css';
import { rich, t } from '../messages/lazyAi';
type Props = {
  config: ProviderConfig;
  onChange: (config: ProviderConfig) => void;
};
/** Compact editable endpoint fields and an explicit remote/plaintext warning. */
export function ProviderFields({ config, onChange }: Props): React.JSX.Element {
  return (
    <>
      <TextField
        label={t('providerName')}
        value={config.name}
        maxLength={80}
        required
        onChange={(event) => {
          onChange({ ...config, name: event.target.value });
        }}
      />
      <SelectField
        label={t('providerType')}
        value={config.kind}
        options={PROVIDER_KINDS}
        onValueChange={(kind) => {
          onChange({
            ...config,
            kind,
            baseUrl:
              PROVIDER_KINDS.find((item) => item.value === kind)?.url ?? '',
            allowInsecureRemote: false,
          });
        }}
      />
      {isCliProvider(config.kind) ? (
        <CodexFields config={config} onChange={onChange} />
      ) : (
        <EndpointFields config={config} onChange={onChange} />
      )}
    </>
  );
}
function EndpointFields({ config, onChange }: Props): React.JSX.Element {
  return (
    <>
      <TextField
        label={t('baseUrl')}
        value={config.baseUrl}
        maxLength={2048}
        required
        type="url"
        spellCheck={false}
        onChange={(event) => {
          onChange({ ...config, baseUrl: event.target.value });
        }}
      />
      <label>
        <input
          type="checkbox"
          checked={config.allowInsecureRemote}
          onChange={(event) => {
            onChange({ ...config, allowInsecureRemote: event.target.checked });
          }}
        />{' '}
        {t('allowRemoteLocal')}
      </label>
      <p className={styles.notice}>{t('warningRemoteEndpoint')}</p>
    </>
  );
}
/** The CLI keeps its own sign-in; only an optional executable path is saved. */
function CodexFields({ config, onChange }: Props): React.JSX.Element {
  return (
    <>
      <TextField
        label={t('codexExecutableOptional')}
        value={config.baseUrl}
        maxLength={1024}
        spellCheck={false}
        placeholder={t('foundPathWhen')}
        onChange={(event) => {
          onChange({ ...config, baseUrl: event.target.value });
        }}
      />
      <p className={styles.notice}>
        {rich('openrowsStartsCode', { code: <code>codex app-server</code> })}
      </p>
    </>
  );
}
