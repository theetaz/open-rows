// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, SelectField, TextField } from '@openrows/ui';
import type { ServerConfig, McpSecretInput } from '@openrows/ipc';
/** Credentials exist only in this unsaved form and are cleared after keychain storage. */
export function McpServerForm(props: {
  save: (server: ServerConfig, secrets: McpSecretInput) => Promise<void>;
  cancel: () => void;
}): React.JSX.Element {
  const controls = useForm(props);
  const { name, address, busy, failure, submit } = controls;
  return (
    <form
      aria-label="Add MCP server"
      onSubmit={(event) => {
        event.preventDefault();
        void submit();
      }}
    >
      <p>
        User servers can access your computer or their own services. Enable only
        trusted tools. Put secrets in environment/headers, never commands,
        arguments or URLs.
      </p>
      <ServerFields controls={controls} />
      <Button type="submit" disabled={busy || !name.trim() || !address.trim()}>
        Save MCP server
      </Button>
      <Button type="button" disabled={busy} onClick={props.cancel}>
        Cancel
      </Button>
      {failure && <p role="alert">{failure}</p>}
    </form>
  );
}
function stringMap(text: string): Record<string, string> {
  const value: unknown = JSON.parse(text);
  if (
    !value ||
    typeof value !== 'object' ||
    Array.isArray(value) ||
    !Object.values(value).every((item: unknown) => typeof item === 'string')
  )
    throw new Error('invalid');
  return value as Record<string, string>;
}

function useForm(props: {
  save: (server: ServerConfig, secrets: McpSecretInput) => Promise<void>;
  cancel: () => void;
}) {
  const [name, setName] = useState('');
  const [kind, setKind] = useState('stdio');
  const [address, setAddress] = useState('');
  const [args, setArgs] = useState('[]');
  const [env, setEnv] = useState('{}');
  const [headers, setHeaders] = useState('{}');
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState('');
  const submit = async (): Promise<void> => {
    setBusy(true);
    setFailure('');
    try {
      const arguments_: unknown = JSON.parse(args);
      if (
        !Array.isArray(arguments_) ||
        !arguments_.every((arg: unknown) => typeof arg === 'string')
      )
        throw new Error('invalid');
      const server: ServerConfig = {
        id: crypto.randomUUID(),
        name,
        enabledTools: [],
        endpoint:
          kind === 'stdio'
            ? { kind: 'stdio', command: address, args: arguments_ }
            : { kind: 'http', url: address },
      };
      await props.save(server, {
        env: stringMap(env),
        headers: stringMap(headers),
      });
      setEnv('{}');
      setHeaders('{}');
      props.cancel();
    } catch {
      setFailure(
        'Could not save server. Check the fields and keychain access.',
      );
    } finally {
      setBusy(false);
    }
  };
  return {
    name,
    kind,
    address,
    args,
    env,
    headers,
    busy,
    failure,
    submit,
    setName,
    setKind,
    setAddress,
    setArgs,
    setEnv,
    setHeaders,
  };
}
function ServerFields({
  controls,
}: {
  controls: ReturnType<typeof useForm>;
}): React.JSX.Element {
  const {
    name,
    kind,
    address,
    args,
    env,
    headers,
    busy,
    setName,
    setKind,
    setAddress,
    setArgs,
    setEnv,
    setHeaders,
  } = controls;
  return (
    <>
      <TextField
        label="MCP server name"
        value={name}
        onChange={inputChange(setName)}
        disabled={busy}
      />
      <SelectField
        label="MCP transport"
        value={kind}
        options={[
          { value: 'stdio', label: 'Stdio executable' },
          { value: 'http', label: 'Streamable HTTP' },
        ]}
        onValueChange={setKind}
        disabled={busy}
      />
      <TextField
        label={kind === 'stdio' ? 'MCP executable' : 'MCP HTTP URL'}
        value={address}
        onChange={inputChange(setAddress)}
        disabled={busy}
      />
      {kind === 'stdio' && (
        <TextField
          label="MCP arguments (JSON array)"
          value={args}
          onChange={inputChange(setArgs)}
          disabled={busy}
        />
      )}
      <TextField
        label="MCP environment (secret JSON object)"
        type="password"
        value={env}
        onChange={inputChange(setEnv)}
        disabled={busy}
      />
      <TextField
        label="MCP headers (secret JSON object)"
        type="password"
        value={headers}
        onChange={inputChange(setHeaders)}
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
