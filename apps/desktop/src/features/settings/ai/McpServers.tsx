// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, Icon } from '@openrows/ui';
import type { McpClientTool, ServerConfig } from '@openrows/ipc';
import { McpServerForm } from './McpServerForm';
import { useMcpServers } from './useMcpServers';
import type { McpServersControls as Controls } from './useMcpServers';
import styles from './Ai.module.css';
import { t } from '../messages/lazyAi';
/** Explicit user-server management; mounting only loads public metadata. */
export function McpServers(): React.JSX.Element {
  const controls = useMcpServers();
  const [editing, setEditing] = useState<ServerConfig | 'new' | null>(null);
  const close = (): void => {
    setEditing(null);
  };
  return (
    <section aria-label={t('userMcpServers')} className={styles.block}>
      <div className={styles.sectionHead}>
        <h3>{t('userMcpServers')}</h3>
        <Button
          size="sm"
          disabled={controls.busy || editing !== null}
          onClick={() => {
            setEditing('new');
          }}
        >
          <Icon name="plus" />
          {t('addMcpServer')}
        </Button>
      </div>
      {editing && (
        <McpServerForm
          key={editing === 'new' ? 'new' : editing.id}
          {...(editing === 'new' ? {} : { initial: editing })}
          failure={controls.failures.form ?? ''}
          save={controls.save}
          close={close}
        />
      )}
      {controls.servers.length > 0 && (
        <div className={[styles.group, styles.withIcons].join(' ')}>
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
        </div>
      )}
      {controls.failures[''] && <p role="alert">{controls.failures['']}</p>}
      <p className={styles.foot}>{t('toolsOwnMcp')}</p>
    </section>
  );
}

function endpointText(server: ServerConfig): string {
  const endpoint = server.endpoint;
  return endpoint.kind === 'http'
    ? t('streamableHttpUrl', { url: endpoint.url })
    : t('stdioValue', {
        value: [endpoint.command, ...endpoint.args].join(' '),
      });
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
    <section
      aria-label={server.name}
      className={[styles.row, styles.provider].join(' ')}
    >
      <span className={styles.rowIcon}>
        <Icon name="plug" />
      </span>
      <div className={styles.rowBody}>
        <strong>{server.name}</strong>
        <p className={styles.mono}>{endpointText(server)}</p>
      </div>
      <div className={styles.accessory}>
        <Button
          size="sm"
          disabled={disabled}
          aria-label={t('discoverNameTools', { name: server.name })}
          onClick={() => void controls.discover(server)}
        >
          {t('discoverTools')}
        </Button>
        <Button
          size="sm"
          disabled={disabled}
          aria-label={t('editName', { name: server.name })}
          onClick={props.edit}
        >
          {t('edit')}
        </Button>
        <Button
          size="sm"
          variant="destructive"
          disabled={disabled}
          aria-label={t('removeName', { name: server.name })}
          onClick={() => void controls.remove(server)}
        >
          {t('remove')}
        </Button>
      </div>
      <ServerTools
        server={server}
        controls={controls}
        disabled={disabled}
        tools={tools}
        discovered={discovered}
        failure={failure}
      />
    </section>
  );
}

/** The tool chips and the discovery state under a server row. */
function ServerTools(props: {
  server: ServerConfig;
  controls: Controls;
  disabled: boolean;
  tools: McpClientTool[];
  discovered: McpClientTool[] | undefined;
  failure: string | undefined;
}): React.JSX.Element {
  const { server, controls, disabled, tools, discovered, failure } = props;
  return (
    <div className={styles.rowFull}>
      {tools.length > 0 && (
        <div className={styles.tools}>
          {tools.map((tool) => (
            <ToolToggle
              key={tool.name}
              label={`${server.name}: ${tool.name}`}
              text={tool.name}
              checked={server.enabledTools.includes(tool.name)}
              disabled={disabled}
              change={(enabled) => {
                void controls.toggle(server, tool.name, enabled);
              }}
            />
          ))}
        </div>
      )}
      {!discovered && (
        <p role="status" className={styles.rowSub}>
          {server.enabledTools.length
            ? t('showingEnabledTools')
            : t('discoverToolsChoose')}
        </p>
      )}
      {discovered?.length === 0 && (
        <p role="status" className={styles.rowSub}>
          {t('serverOffersNo')}
        </p>
      )}
      {failure && <p role="alert">{failure}</p>}
    </div>
  );
}

function ToolToggle(props: {
  label: string;
  text: string;
  checked: boolean;
  disabled: boolean;
  change: (enabled: boolean) => void;
}): React.JSX.Element {
  return (
    <label className={styles.chip}>
      <input
        type="checkbox"
        aria-label={props.label}
        checked={props.checked}
        disabled={props.disabled}
        onChange={(event) => {
          props.change(event.target.checked);
        }}
      />
      {props.text}
    </label>
  );
}
