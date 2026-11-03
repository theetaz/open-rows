// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, SelectField, TextField } from '@openrows/ui';
import type { ServerConfig } from '@openrows/ipc';
import type { SecretChange } from './useMcpServers';
import styles from './Ai.module.css';
import { t } from '../messages/lazyAi';
type Save = (server: ServerConfig, secrets: SecretChange) => Promise<boolean>;
/**
 * Add or edit one server. Credentials exist only in this unsaved form; when
 * editing, empty secret fields keep the values already in the keychain.
 */
export function McpServerForm(props: {
  initial?: ServerConfig;
  failure: string;
  save: Save;
  close: () => void;
}): React.JSX.Element {
  const controls = useForm(props);
  const editing = !!props.initial;
  const label = editing
    ? t('editName', { name: props.initial?.name ?? '' })
    : t('addMcpServer');
  return (
    <form
      aria-label={label}
      className={styles.form}
      onSubmit={(event) => {
        event.preventDefault();
        void controls.submit();
      }}
    >
      <p className={styles.notice}>{t('userServersCan')}</p>
      <ServerFields controls={controls} editing={editing} />
      <div className={styles.actions}>
        <Button
          type="submit"
          disabled={
            controls.busy || !controls.name.trim() || !controls.address.trim()
          }
        >
          {t('saveMcpServer')}
        </Button>
        <Button type="button" disabled={controls.busy} onClick={props.close}>
          {t('cancel')}
        </Button>
      </div>
      {(controls.failure || props.failure) && (
        <p role="alert">{controls.failure || props.failure}</p>
      )}
    </form>
  );
}
function stringMap(text: string): Record<string, string> | null {
  const value: unknown = JSON.parse(text);
  if (
    !value ||
    typeof value !== 'object' ||
    Array.isArray(value) ||
    !Object.values(value).every((item: unknown) => typeof item === 'string')
  )
    return null;
  return value as Record<string, string>;
}
function parsed<T>(text: string, read: (text: string) => T | null): T | null {
  try {
    return read(text);
  } catch {
    return null;
  }
}
function argumentList(text: string): string[] | null {
  const value: unknown = JSON.parse(text);
  return Array.isArray(value) &&
    value.every((arg: unknown) => typeof arg === 'string')
    ? value
    : null;
}
function secretsOf(env: string, headers: string): SecretChange | string {
  if (!env.trim() && !headers.trim()) return null;
  const values = {
    env: parsed(env.trim() || '{}', stringMap),
    headers: parsed(headers.trim() || '{}', stringMap),
  };
  if (!values.env || !values.headers)
    return t('environmentHeadersMust', { example: '{"API_TOKEN": "…"}' });
  return { env: values.env, headers: values.headers };
}
function useForm(props: {
  initial?: ServerConfig;
  save: Save;
  close: () => void;
}) {
  const endpoint = props.initial?.endpoint;
  const [name, setName] = useState(props.initial?.name ?? '');
  const [kind, setKind] = useState<string>(endpoint?.kind ?? 'stdio');
  const [address, setAddress] = useState(
    endpoint?.kind === 'http' ? endpoint.url : (endpoint?.command ?? ''),
  );
  const [args, setArgs] = useState(
    JSON.stringify(endpoint?.kind === 'stdio' ? endpoint.args : []),
  );
  const [env, setEnv] = useState('');
  const [headers, setHeaders] = useState('');
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState('');
  const submit = async (): Promise<void> => {
    const list = parsed(args, argumentList);
    const secrets = secretsOf(env, headers);
    if (!list || typeof secrets === 'string') {
      setFailure(
        typeof secrets === 'string' ? secrets : t('argumentsMustJson'),
      );
      return;
    }
    setFailure('');
    setBusy(true);
    const saved = await props.save(
      {
        id: props.initial?.id ?? crypto.randomUUID(),
        name: name.trim(),
        enabledTools: props.initial?.enabledTools ?? [],
        endpoint:
          kind === 'stdio'
            ? { kind: 'stdio', command: address.trim(), args: list }
            : { kind: 'http', url: address.trim() },
      },
      secrets,
    );
    setBusy(false);
    if (saved) props.close();
  };
  return {
    ...{ name, kind, address, args, env, headers, busy, failure, submit },
    ...{ setName, setKind, setAddress, setArgs, setEnv, setHeaders },
  };
}
function ServerFields({
  controls,
  editing,
}: {
  controls: ReturnType<typeof useForm>;
  editing: boolean;
}): React.JSX.Element {
  const { kind, busy } = controls;
  const keep = editing
    ? t('unchangedStoredKeychain')
    : '{"NAME": "value"} — kept in the keychain';
  return (
    <>
      <TextField
        label={t('mcpServerName')}
        value={controls.name}
        onChange={inputChange(controls.setName)}
        disabled={busy}
      />
      <SelectField
        label={t('mcpTransport')}
        value={kind}
        options={[
          { value: 'stdio', label: t('stdioExecutable') },
          { value: 'http', label: t('streamableHttp') },
        ]}
        onValueChange={controls.setKind}
        disabled={busy}
      />
      <TextField
        label={kind === 'stdio' ? t('mcpExecutable') : t('mcpHttpUrl')}
        value={controls.address}
        onChange={inputChange(controls.setAddress)}
        disabled={busy}
      />
      {kind === 'stdio' && (
        <TextField
          label={t('mcpArgumentsJson')}
          value={controls.args}
          onChange={inputChange(controls.setArgs)}
          disabled={busy}
        />
      )}
      <TextField
        label={t('mcpEnvironmentSecret')}
        placeholder={keep}
        type="password"
        value={controls.env}
        onChange={inputChange(controls.setEnv)}
        disabled={busy}
      />
      <TextField
        label={t('mcpHeadersSecret')}
        placeholder={keep}
        type="password"
        value={controls.headers}
        onChange={inputChange(controls.setHeaders)}
        disabled={busy}
      />
    </>
  );
}

function inputChange(
  set: (value: string) => void,
): (event: React.ChangeEvent<HTMLInputElement>) => void {
  return (event) => {
    set(event.target.value);
  };
}
