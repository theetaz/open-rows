// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import {
  deleteMcpServer,
  discoverMcpTools,
  listMcpServers,
  saveMcpServers,
  setMcpServerSecrets,
} from '@openrows/ipc';
import type {
  McpClientTool,
  McpSecretInput,
  ServerConfig,
} from '@openrows/ipc';
import { mcpFailure } from './mcpFailure';
type Tools = Partial<Record<string, McpClientTool[]>>;
type SetTools = React.Dispatch<React.SetStateAction<Tools>>;
type SetServers = React.Dispatch<React.SetStateAction<ServerConfig[]>>;
/** Secrets to store; `null` keeps the server's stored values. */
export type SecretChange = McpSecretInput | null;
/** Failures are keyed by server id; `form` is the add/edit form, `''` the list. */
type Run = (
  key: string,
  label: string,
  work: () => Promise<void>,
) => Promise<boolean>;

function useServerList(
  report: (key: string, message: string) => void,
): [ServerConfig[], SetServers, Tools, SetTools] {
  const [servers, setServers] = useState<ServerConfig[]>([]);
  const [tools, setTools] = useState<Tools>({});
  useEffect(() => {
    let live = true;
    listMcpServers().then(
      (list) => {
        if (!live) return;
        setServers(list.servers);
        setTools(list.tools);
      },
      (error: unknown) => {
        if (live) report('', mcpFailure(error, 'MCP servers'));
      },
    );
    return () => {
      live = false;
    };
  }, [report]);
  return [servers, setServers, tools, setTools];
}

function useRunner(): {
  busy: boolean;
  failures: Partial<Record<string, string>>;
  report: (key: string, message: string) => void;
  run: Run;
} {
  const [busy, setBusy] = useState(false);
  const [failures, setFailures] = useState<Partial<Record<string, string>>>({});
  const [report] = useState(() => (key: string, message: string): void => {
    setFailures((previous) => ({ ...previous, [key]: message }));
  });
  const run: Run = async (key, label, work) => {
    setBusy(true);
    report(key, '');
    try {
      await work();
      return true;
    } catch (error) {
      report(key, mcpFailure(error, label));
      return false;
    } finally {
      setBusy(false);
    }
  };
  return { busy, failures, report, run };
}

const sameEndpoint = (left: ServerConfig, right: ServerConfig): boolean =>
  JSON.stringify(left.endpoint) === JSON.stringify(right.endpoint);

/** Add or edit; a new server is removed again if its secrets cannot be stored. */
function saver(input: {
  servers: ServerConfig[];
  setServers: SetServers;
  setTools: SetTools;
  run: Run;
}) {
  const { servers, setServers, setTools } = input;
  return (server: ServerConfig, secrets: SecretChange) =>
    input.run('form', server.name, async () => {
      const previous = servers.find((item) => item.id === server.id);
      const next = previous
        ? servers.map((item) => (item.id === server.id ? server : item))
        : [...servers, server];
      await saveMcpServers(next);
      setServers(next);
      const stale = !previous || !sameEndpoint(previous, server) || !!secrets;
      if (stale) setTools((tools) => ({ ...tools, [server.id]: undefined }));
      if (!secrets) return;
      try {
        await setMcpServerSecrets(server.id, secrets);
      } catch (error) {
        if (!previous) {
          await deleteMcpServer(server.id);
          setServers(servers);
        }
        throw error;
      }
    });
}

function toggled(
  servers: ServerConfig[],
  server: ServerConfig,
  name: string,
  enabled: boolean,
): ServerConfig[] {
  return servers.map((item) =>
    item.id === server.id
      ? {
          ...item,
          enabledTools: enabled
            ? [...item.enabledTools, name]
            : item.enabledTools.filter((tool) => tool !== name),
        }
      : item,
  );
}

type Act = (server: ServerConfig) => Promise<boolean>;
/** What the MCP settings render and do. */
export type McpServersControls = {
  servers: ServerConfig[];
  tools: Tools;
  busy: boolean;
  failures: Partial<Record<string, string>>;
  save: (server: ServerConfig, secrets: SecretChange) => Promise<boolean>;
  remove: Act;
  discover: Act;
  toggle: (
    server: ServerConfig,
    name: string,
    enabled: boolean,
  ) => Promise<boolean>;
};
/** User server list, last discovered catalogue and one failure per server. */
export function useMcpServers(): McpServersControls {
  const { busy, failures, report, run } = useRunner();
  const [servers, setServers, tools, setTools] = useServerList(report);
  return {
    servers,
    tools,
    busy,
    failures,
    save: saver({ servers, setServers, setTools, run }),
    remove: (server: ServerConfig) =>
      run(server.id, server.name, async () => {
        await deleteMcpServer(server.id);
        setServers((list) => list.filter((item) => item.id !== server.id));
      }),
    discover: (server: ServerConfig) =>
      run(server.id, server.name, async () => {
        setTools((previous) => ({ ...previous, [server.id]: undefined }));
        const found = await discoverMcpTools(server.id);
        setTools((previous) => ({ ...previous, [server.id]: found }));
      }),
    toggle: (server: ServerConfig, name: string, enabled: boolean) =>
      run(server.id, server.name, async () => {
        const next = toggled(servers, server, name, enabled);
        await saveMcpServers(next);
        setServers(next);
      }),
  };
}
