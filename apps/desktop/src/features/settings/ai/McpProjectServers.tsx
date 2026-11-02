// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import type { ProjectServerView } from '@openrows/ipc';
import { useProjectMcpServers } from './useProjectMcpServers';
import type {
  ProjectServersControls as Controls,
  Note,
} from './useProjectMcpServers';
import styles from './Ai.module.css';

const STATUS = {
  untrusted: 'Not trusted — it will not start',
  changed: 'Definition changed — trust and stored values were removed',
  trusted: 'Trusted on this device',
} as const;

/** Servers declared by loaded Projects; each must be trusted before it runs. */
export function McpProjectServers(): React.JSX.Element {
  const controls = useProjectMcpServers();
  return (
    <section aria-label="Project MCP servers">
      <h3>Project MCP servers</h3>
      <p className={styles.notice}>
        A project’s <code>.openrows/project.yaml</code> can declare MCP servers.
        They never start until you trust the exact definition below on this
        device; any change to the file removes that trust. Values for their
        variables and headers are typed into a secure system dialog and kept in
        the OS keychain, never in the project.
      </p>
      {controls.servers.length === 0 && (
        <p role="status">
          Open a project that declares MCP servers to see them.
        </p>
      )}
      {controls.servers.map((view) => (
        <ProjectServerRow key={view.key} view={view} controls={controls} />
      ))}
      <NoteLine note={controls.notes['']} />
    </section>
  );
}

function ProjectServerRow(props: {
  view: ProjectServerView;
  controls: Controls;
}): React.JSX.Element {
  const { view, controls } = props;
  const label = `${view.project}: ${view.server.name}`;
  return (
    <section aria-label={label} className={styles.provider}>
      <div className={styles.heading}>
        <strong>{label}</strong>
        <span role="status">{STATUS[view.trust]}</span>
      </div>
      <Definition view={view} />
      {view.trust === 'trusted' ? (
        <TrustedActions view={view} controls={controls} label={label} />
      ) : (
        <div className={styles.actions}>
          <Button
            disabled={controls.busy}
            onClick={() => void controls.trust(view)}
          >
            Trust {label}
          </Button>
        </div>
      )}
      <NoteLine note={controls.notes[view.key]} />
    </section>
  );
}

/** Failures are announced as alerts; outcomes politely. */
function NoteLine({
  note,
}: {
  note: Note | undefined;
}): React.JSX.Element | null {
  if (!note?.text) return null;
  return <p role={note.failed ? 'alert' : 'status'}>{note.text}</p>;
}

/** Exactly what will run: command and arguments or URL, and required names. */
function Definition({ view }: { view: ProjectServerView }): React.JSX.Element {
  const { endpoint, env = [], headers = [] } = view.server;
  return (
    <dl className={styles.definition}>
      <dt>Project folder</dt>
      <dd>{view.root}</dd>
      {endpoint.kind === 'stdio' ? (
        <>
          <dt>Command</dt>
          <dd>
            <code>{endpoint.command}</code>
          </dd>
          <dt>Arguments</dt>
          <dd>
            <code>{JSON.stringify(endpoint.args)}</code>
          </dd>
        </>
      ) : (
        <>
          <dt>URL</dt>
          <dd>
            <code>{endpoint.url}</code>
          </dd>
        </>
      )}
      <dt>Environment variables</dt>
      <dd>{env.length ? env.join(', ') : 'none'}</dd>
      <dt>Headers</dt>
      <dd>{headers.length ? headers.join(', ') : 'none'}</dd>
    </dl>
  );
}

function TrustedActions(props: {
  view: ProjectServerView;
  controls: Controls;
  label: string;
}): React.JSX.Element {
  const { view, controls, label } = props;
  const names = [...(view.server.env ?? []), ...(view.server.headers ?? [])];
  return (
    <>
      <div className={styles.actions}>
        <Button
          disabled={controls.busy}
          onClick={() => void controls.discover(view)}
        >
          Discover {label} tools
        </Button>
        {names.map((name) => (
          <Button
            key={name}
            disabled={controls.busy}
            onClick={() => void controls.secret(view, name)}
          >
            Set {name}…
          </Button>
        ))}
        <Button
          disabled={controls.busy}
          onClick={() => void controls.revoke(view)}
        >
          Revoke trust in {label}
        </Button>
      </div>
      {view.tools.length === 0 && (
        <p className={styles.notice}>
          Discover tools to choose which ones the assistant may use.
        </p>
      )}
      <div className={styles.form}>
        {view.tools.map((tool) => (
          <label key={tool.name}>
            <input
              type="checkbox"
              checked={view.enabledTools.includes(tool.name)}
              disabled={controls.busy}
              onChange={(event) =>
                void controls.toggle(view, tool.name, event.target.checked)
              }
            />{' '}
            {label}: {tool.name}
          </label>
        ))}
      </div>
    </>
  );
}
