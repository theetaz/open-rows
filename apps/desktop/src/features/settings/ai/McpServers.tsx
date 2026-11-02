// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button } from '@openrows/ui';
import type { McpClientTool, ServerConfig } from '@openrows/ipc';
import { McpServerForm } from './McpServerForm';
import { useMcpServers } from './useMcpServers';
import type { McpServersControls as Controls } from './useMcpServers';
import styles from './Ai.module.css';
/** Explicit user-server management; mounting only loads public metadata. */
export function McpServers(): React.JSX.Element {
  const controls = useMcpServers();
  const [editing, setEditing] = useState<ServerConfig | 'new' | null>(null);
  const close = (): void => {
    setEditing(null);
  };
  return (
    <section aria-label="User MCP servers">
      <h3>User MCP servers</h3>
      <p className={styles.notice}>
        Tools from your own MCP servers can be offered to the assistant. Each
        tool starts disabled, and its first use in a session asks you to confirm
        the exact arguments. Sending arguments or results to a provider needs
        the connection’s SampleRows sharing level; Local only refuses them.
      </p>
      <Button
        disabled={controls.busy || editing !== null}
        onClick={() => {
          setEditing('new');
        }}
      >
        Add MCP server
      </Button>
      {editing && (
        <McpServerForm
          key={editing === 'new' ? 'new' : editing.id}
          {...(editing === 'new' ? {} : { initial: editing })}
          failure={controls.failures.form ?? ''}
          save={controls.save}
          close={close}
        />
      )}
      {controls.servers.map((server) => (
        <ServerRow
          key={server.id}
          server={server}
          controls={controls}
          disabled={controls.busy || editing !== null}
          edit={() => {
            setEditing(server);
          }}
        />
      ))}
      {controls.failures[''] && <p role="alert">{controls.failures['']}</p>}
    </section>
  );
}

function endpointText(server: ServerConfig): string {
  const endpoint = server.endpoint;
  return endpoint.kind === 'http'
    ? `Streamable HTTP · ${endpoint.url}`
    : `Stdio · ${[endpoint.command, ...endpoint.args].join(' ')}`;
}

function ServerRow(props: {
  server: ServerConfig;
  controls: Controls;
  disabled: boolean;
  edit: () => void;
}): React.JSX.Element {
  const { server, controls, disabled } = props;
  const discovered = controls.tools[server.id];
  const tools: McpClientTool[] =
    discovered ?? server.enabledTools.map((name) => ({ name, schema: '' }));
  const failure = controls.failures[server.id];
  return (
    <section aria-label={server.name} className={styles.provider}>
      <div className={styles.heading}>
        <strong>{server.name}</strong>
      </div>
      <p>{endpointText(server)}</p>
      <div className={styles.actions}>
        <Button
          disabled={disabled}
          onClick={() => void controls.discover(server)}
        >
          Discover {server.name} tools
        </Button>
        <Button disabled={disabled} onClick={props.edit}>
          Edit {server.name}
        </Button>
        <Button
          disabled={disabled}
          onClick={() => void controls.remove(server)}
        >
          Remove {server.name}
        </Button>
      </div>
      {!discovered && (
        <p role="status" className={styles.notice}>
          {server.enabledTools.length
            ? 'Showing enabled tools. Discover tools to see the full list.'
            : 'Discover tools to choose which ones the assistant may use.'}
        </p>
      )}
      {discovered?.length === 0 && (
        <p role="status" className={styles.notice}>
          This server offers no tools.
        </p>
      )}
      <div className={styles.form}>
        {tools.map((tool) => (
          <ToolToggle
            key={tool.name}
            label={`${server.name}: ${tool.name}`}
            checked={server.enabledTools.includes(tool.name)}
            disabled={disabled}
            change={(enabled) => {
              void controls.toggle(server, tool.name, enabled);
            }}
          />
        ))}
      </div>
      {failure && <p role="alert">{failure}</p>}
    </section>
  );
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
