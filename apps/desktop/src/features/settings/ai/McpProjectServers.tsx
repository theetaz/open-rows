// SPDX-License-Identifier: Apache-2.0
import { Button, Icon } from '@openrows/ui';
import type { ProjectServerView } from '@openrows/ipc';
import { useProjectMcpServers } from './useProjectMcpServers';
import type {
  ProjectServersControls as Controls,
  Note,
} from './useProjectMcpServers';
import styles from './Ai.module.css';
import { rich, t } from '../messages/lazyAi';

const STATUS = {
  untrusted: t('notTrustedNot'),
  changed: t('definitionChangedTrust'),
  trusted: t('trustedDevice'),
} as const;

/** Servers declared by loaded Projects; each must be trusted before it runs. */
export function McpProjectServers(): React.JSX.Element {
  const controls = useProjectMcpServers();
  return (
    <section aria-label={t('projectMcpServers')} className={styles.block}>
      <h3 className={styles.title}>{t('projectMcpServers')}</h3>
      {controls.servers.length > 0 && (
        <div className={[styles.group, styles.withIcons].join(' ')}>
          {controls.servers.map((view) => (
            <ProjectServerRow key={view.key} view={view} controls={controls} />
          ))}
        </div>
      )}
      {controls.servers.length === 0 && (
        <p role="status" className={styles.foot}>
          {t('openProjectDeclares')}
        </p>
      )}
      <NoteLine note={controls.notes['']} />
      <p className={styles.foot}>
        {rich('projectSCode', { code: <code>.openrows/project.yaml</code> })}
      </p>
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
    <section
      aria-label={label}
      className={[styles.row, styles.provider].join(' ')}
    >
      <span
        className={styles.rowIcon}
        data-tone={view.trust === 'trusted' ? undefined : 'warning'}
      >
        <Icon name={view.trust === 'trusted' ? 'plug' : 'warning'} />
      </span>
      <div className={styles.rowBody}>
        <strong>{label}</strong>
        <p role="status">{STATUS[view.trust]}</p>
      </div>
      {view.trust !== 'trusted' && (
        <div className={styles.accessory}>
          <Button
            size="sm"
            disabled={controls.busy}
            aria-label={t('trustLabel', { label })}
            onClick={() => void controls.trust(view)}
          >
            {t('trust')}
          </Button>
        </div>
      )}
      <div className={styles.rowFull}>
        <Definition view={view} />
        {view.trust === 'trusted' && (
          <TrustedActions view={view} controls={controls} label={label} />
        )}
        <NoteLine note={controls.notes[view.key]} />
      </div>
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
      <dt>{t('projectFolder')}</dt>
      <dd>{view.root}</dd>
      {endpoint.kind === 'stdio' ? (
        <>
          <dt>{t('command')}</dt>
          <dd>
            <code>{endpoint.command}</code>
          </dd>
          <dt>{t('arguments')}</dt>
          <dd>
            <code>{JSON.stringify(endpoint.args)}</code>
          </dd>
        </>
      ) : (
        <>
          <dt>{t('url')}</dt>
          <dd>
            <code>{endpoint.url}</code>
          </dd>
        </>
      )}
      <dt>{t('environmentVariables')}</dt>
      <dd>{env.length ? env.join(', ') : t('none')}</dd>
      <dt>{t('headers')}</dt>
      <dd>{headers.length ? headers.join(', ') : t('none')}</dd>
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
          size="sm"
          disabled={controls.busy}
          aria-label={t('discoverLabelTools', { label })}
          onClick={() => void controls.discover(view)}
        >
          {t('discoverTools')}
        </Button>
        {names.map((name) => (
          <Button
            key={name}
            size="sm"
            variant="ghost"
            disabled={controls.busy}
            onClick={() => void controls.secret(view, name)}
          >
            {t('setName', { name })}
          </Button>
        ))}
        <Button
          size="sm"
          variant="destructive"
          disabled={controls.busy}
          onClick={() => void controls.revoke(view)}
        >
          {t('revokeTrustLabel', { label })}
        </Button>
      </div>
      {view.tools.length === 0 && (
        <p className={styles.rowSub}>{t('discoverToolsChoose')}</p>
      )}
      <div className={styles.tools}>
        {view.tools.map((tool) => (
          <label key={tool.name} className={styles.chip}>
            <input
              type="checkbox"
              aria-label={`${label}: ${tool.name}`}
              checked={view.enabledTools.includes(tool.name)}
              disabled={controls.busy}
              onChange={(event) =>
                void controls.toggle(view, tool.name, event.target.checked)
              }
            />
            {tool.name}
          </label>
        ))}
      </div>
    </>
  );
}
