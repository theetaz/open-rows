// SPDX-License-Identifier: Apache-2.0
import { SelectField, TextField } from '@openrows/ui';
import type { ProviderConfig } from '@openrows/ipc';
import { PROVIDER_KINDS } from './aiSettings';
import styles from './Ai.module.css';
/** Compact editable endpoint fields and an explicit remote/plaintext warning. */
export function ProviderFields(props: {
  config: ProviderConfig;
  onChange: (config: ProviderConfig) => void;
}): React.JSX.Element {
  const { config, onChange } = props;
  return (
    <>
      <TextField
        label="Provider name"
        value={config.name}
        maxLength={80}
        required
        onChange={(event) => {
          onChange({ ...config, name: event.target.value });
        }}
      />
      <SelectField
        label="Provider type"
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
      <TextField
        label="Base URL"
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
        Allow remote local servers / unencrypted remote HTTP
      </label>
      <p className={styles.notice}>
        Warning: a remote endpoint receives requests and API keys. HTTP sends
        them without encryption. Local servers are restricted to loopback by
        default.
      </p>
    </>
  );
}
