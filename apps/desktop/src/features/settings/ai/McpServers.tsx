// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { Button } from '@openrows/ui';
import {
  listMcpServers,
  saveMcpServers,
  deleteMcpServer,
  discoverMcpTools,
  setMcpServerSecrets,
} from '@openrows/ipc';
import type {
  McpClientTool,
  ServerConfig,
  McpSecretInput,
} from '@openrows/ipc';
import { McpServerForm } from './McpServerForm';
/** Explicit user-server management; mounting only loads public metadata. */
export function McpServers(): React.JSX.Element {
  const {
    servers,
    tools,
    adding,
    busy,
    failure,
    setAdding,
    setTools,
    setServers,
    save,
    toggle,
    run,
  } = useServers();
  return (
    <section aria-label="User MCP servers">
      <h3>User MCP servers</h3>
      <p>
        Tools start disabled. First use asks for confirmation each session.
        External arguments/results require SampleRows sharing; Local only
        refuses external tools. Discover tools once after restarting OpenRows.
      </p>
      <Button
        disabled={busy || adding}
        onClick={() => {
          setAdding(true);
        }}
      >
        Add MCP server
      </Button>
      {adding && (
        <McpServerForm
          save={save}
          cancel={() => {
            setAdding(false);
          }}
        />
      )}
      {servers.map((server) => (
        <ServerRow
          key={server.id}
          server={server}
          servers={servers}
          tools={
            tools[server.id] ??
            server.enabledTools.map((name) => ({ name, schema: '' }))
          }
          busy={busy || adding}
          run={run}
          setTools={setTools}
          setServers={setServers}
          toggle={toggle}
        />
      ))}
      {failure && <p role="alert">{failure}</p>}
    </section>
  );
}

function useServers() {
  const [servers, setServers, loadFailure] = useServerList();
  const [tools, setTools] = useState<Record<string, McpClientTool[]>>({});
  const [adding, setAdding] = useState(false);
  const { busy, failure, run } = useOperation();
  const save = async (
    server: ServerConfig,
    secrets: McpSecretInput,
  ): Promise<void> => {
    await saveMcpServers([...servers, server]);
    setServers([...servers, server]);
    try {
      await setMcpServerSecrets(server.id, secrets);
    } catch (error) {
      await deleteMcpServer(server.id);
      setServers(servers);
      throw error;
    }
  };
  const toggle = async (
    server: ServerConfig,
    name: string,
    enabled: boolean,
  ): Promise<void> => {
    const next = servers.map((item) =>
      item.id === server.id
        ? {
            ...item,
            enabledTools: enabled
              ? [...item.enabledTools, name]
              : item.enabledTools.filter((tool) => tool !== name),
          }
        : item,
    );
    await saveMcpServers(next);
    setServers(next);
  };
  return {
    servers,
    tools,
    adding,
    busy,
    failure: failure || loadFailure,
    setAdding,
    setTools,
    setServers,
    save,
    toggle,
    run,
  };
}
function ServerRow({
  server,
  servers,
  tools,
  busy,
  run,
  setTools,
  setServers,
  toggle,
}: {
  server: ServerConfig;
  servers: ServerConfig[];
  tools: McpClientTool[];
  busy: boolean;
  run: (work: () => Promise<void>) => Promise<void>;
  setTools: React.Dispatch<
    React.SetStateAction<Record<string, McpClientTool[]>>
  >;
  setServers: (servers: ServerConfig[]) => void;
  toggle: (
    server: ServerConfig,
    name: string,
    enabled: boolean,
  ) => Promise<void>;
}): React.JSX.Element {
  return (
    <section aria-label={server.name}>
      <strong>{server.name}</strong> · {server.endpoint.kind}
      <Button
        disabled={busy}
        onClick={() => {
          void run(async () => {
            setTools((previous) => ({ ...previous, [server.id]: [] }));
            const tools = await discoverMcpTools(server.id);
            setTools((previous) => ({ ...previous, [server.id]: tools }));
          });
        }}
      >
        Discover {server.name} tools
      </Button>
      <Button
        disabled={busy}
        onClick={() => {
          void run(async () => {
            await deleteMcpServer(server.id);
            setServers(servers.filter((item) => item.id !== server.id));
          });
        }}
      >
        Remove {server.name}
      </Button>
      {tools.map((tool) => (
        <ToolToggle
          key={tool.name}
          label={`${server.name}: ${tool.name}`}
          checked={server.enabledTools.includes(tool.name)}
          disabled={busy}
          change={(enabled) => {
            void run(() => toggle(server, tool.name, enabled));
          }}
        />
      ))}
    </section>
  );
}

function useServerList(): [
  ServerConfig[],
  React.Dispatch<React.SetStateAction<ServerConfig[]>>,
  string,
] {
  const [servers, setServers] = useState<ServerConfig[]>([]);
  const [failure, setFailure] = useState('');
  useEffect(() => {
    let live = true;
    listMcpServers().then(
      (value) => {
        if (live) setServers(value);
      },
      () => {
        if (live) setFailure('MCP servers unavailable.');
      },
    );
    return () => {
      live = false;
    };
  }, []);
  return [servers, setServers, failure];
}
function useOperation() {
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState('');
  const run = async (work: () => Promise<void>): Promise<void> => {
    setBusy(true);
    setFailure('');
    try {
      await work();
    } catch {
      setFailure(
        'MCP operation failed. Check configuration, AI permission and keychain access.',
      );
    } finally {
      setBusy(false);
    }
  };
  return { busy, failure, run };
}

function ToolToggle(props: {
  label: string;
  checked: boolean;
  disabled: boolean;
  change: (enabled: boolean) => void;
}): React.JSX.Element {
  return (
    <label>
      <input
        type="checkbox"
        checked={props.checked}
        disabled={props.disabled}
        onChange={(event) => {
          props.change(event.target.checked);
        }}
      />{' '}
      {props.label}
    </label>
  );
}
