// SPDX-License-Identifier: Apache-2.0
import { SelectField, TextField } from '@openrows/ui';
import type { ProviderConfig } from '@openrows/ipc';
import { isCliProvider, PROVIDER_KINDS } from './aiSettings';
import styles from './Ai.module.css';
type Props = {
  config: ProviderConfig;
  onChange: (config: ProviderConfig) => void;
};
/** Compact editable endpoint fields and an explicit remote/plaintext warning. */
export function ProviderFields({ config, onChange }: Props): React.JSX.Element {
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
/** The CLI keeps its own sign-in; only an optional executable path is saved. */
function CodexFields({ config, onChange }: Props): React.JSX.Element {
  return (
    <>
      <TextField
        label="Codex executable (optional)"
        value={config.baseUrl}
        maxLength={1024}
        spellCheck={false}
        placeholder="Found on PATH when empty"
        onChange={(event) => {
          onChange({ ...config, baseUrl: event.target.value });
        }}
      />
      <p className={styles.notice}>
        OpenRows starts <code>codex app-server</code> in its own folder with a
        separate sign-in, a read-only sandbox and no shell, file, web or MCP
        tools. Only the database tools your sharing policy allows are offered,
        and every approval Codex asks for is declined.
      </p>
    </>
  );
}
