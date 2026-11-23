// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef } from 'react';
import { Button, Icon } from '@openrows/ui';
import { setProjectConnection } from '@openrows/ipc';
import type { ProfileDto, ProjectDto } from '@openrows/ipc';
import type { MenuEntry } from '@openrows/ui';
import { MenuButton } from '../query/SplitButton';
import type { ProjectHooks } from './projectActions';
import {
  closeProject,
  openProject,
  openProjectDialog,
  reloadProject,
  reportProjectError,
  showProject,
  useProject,
} from './projectStore';
import { baseName } from './treeModel';
import { ProjectConnections } from './ProjectConnections';
import { ProjectTree } from './ProjectTree';
import styles from './ProjectSection.module.css';
import { t } from './messages';

type Props = ProjectHooks & {
  search: string;
  /** The active connection, for Use as Default Connection. */
  active: ProfileDto | undefined;
  profiles: readonly ProfileDto[];
  /** Select the project's default connection when none is active. */
  onConnection: (id: string) => void;
};

const folderName = (root: string): string =>
  baseName(root.replaceAll('\\', '/').replace(/\/+$/, '')) || root;

function setDefault(project: ProjectDto, id: string | null): void {
  setProjectConnection(project.root, id).then(showProject, reportProjectError);
}

/** Open, recent, default connection and close commands of the project menu. */
function projectEntries(
  project: ProjectDto,
  recent: readonly string[],
  active: ProfileDto | undefined,
): MenuEntry[] {
  const others = recent.filter((root) => root !== project.root);
  const connection: MenuEntry = active
    ? {
        kind: 'item',
        label: t('useNameDefault', { name: active.name }),
        checked: project.defaultConnection === active.id,
        run: () => {
          setDefault(project, active.id);
        },
      }
    : {
        kind: 'item',
        label: t('useConnectionDefault'),
        disabled: true,
        run: () => undefined,
      };
  return [
    { kind: 'item', label: t('openProject'), run: openProjectDialog },
    {
      kind: 'submenu',
      label: t('openRecent'),
      entries: others.length
        ? others.map((root) => ({
            kind: 'item' as const,
            label: folderName(root),
            run: () => {
              openProject(root);
            },
          }))
        : [
            {
              kind: 'item',
              label: t('noRecentProjects'),
              disabled: true,
              run: () => undefined,
            },
          ],
    },
    { kind: 'separator' },
    connection,
    {
      kind: 'item',
      label: t('clearDefaultConnection'),
      disabled: project.defaultConnection === null,
      run: () => {
        setDefault(project, null);
      },
    },
    { kind: 'separator' },
    { kind: 'item', label: t('reload'), run: () => void reloadProject() },
    { kind: 'item', label: t('closeProject'), run: closeProject },
  ];
}

/** No project open: Open Project… and the recent projects. */
function NoProject(props: { recent: readonly string[] }): React.JSX.Element {
  return (
    <div className={styles.empty}>
      <p>{t('keepQueriesSql')}</p>
      <Button size="sm" onClick={openProjectDialog}>
        {t('openProject')}
      </Button>
      {props.recent.length > 0 && (
        <div
          className={styles.recent}
          aria-label={t('recentProjects')}
          role="group"
        >
          {props.recent.map((root) => (
            <Button
              key={root}
              size="sm"
              variant="ghost"
              title={root}
              onClick={() => {
                openProject(root);
              }}
            >
              {folderName(root)}
            </Button>
          ))}
        </div>
      )}
    </div>
  );
}

/** Reopen the project's default connection when nothing else is active. */
function useDefaultConnection(project: ProjectDto | null, props: Props): void {
  const latest = useRef({ project, props });
  useEffect(() => {
    latest.current = { project, props };
  });
  const root = project?.root;
  // Only when a project opens, not when the active connection changes.
  useEffect(() => {
    const { project: opened, props: current } = latest.current;
    const id = opened?.defaultConnection;
    if (!root || !id || current.active) return;
    if (current.profiles.some((profile) => profile.id === id))
      current.onConnection(id);
  }, [root]);
}

/**
 * The Queries sidebar's project: its query folders as a tree, the project
 * menu, or Open Project… and recent projects when none is open.
 */
export function ProjectSection(props: Props): React.JSX.Element {
  const { project, recent, error } = useProject();
  useDefaultConnection(project, props);
  useEffect(() => {
    const reload = (): void => {
      void reloadProject();
    };
    window.addEventListener('focus', reload);
    return () => {
      window.removeEventListener('focus', reload);
    };
  }, []);
  return (
    <section aria-label={t('project')}>
      <div className={styles.header}>
        <Icon name="schema" />
        <span className={styles.title} title={project?.root}>
          <strong>{project ? project.name : t('project')}</strong>
          {project && <small>{project.root}</small>}
        </span>
        {project && (
          <MenuButton
            label={t('projectActions')}
            variant="ghost"
            entries={projectEntries(project, recent, props.active)}
          >
            <Icon name="more" />
          </MenuButton>
        )}
      </div>
      {error && (
        <p role="alert" className={styles.error}>
          {error}
        </p>
      )}
      {project && (
        <ProjectConnections
          key={project.root}
          project={project}
          profiles={props.profiles}
        />
      )}
      {project ? (
        <ProjectTree
          key={project.root}
          project={project}
          search={props.search}
          onOpenQuery={props.onOpenQuery}
          onMoved={props.onMoved}
        />
      ) : (
        <NoProject recent={recent} />
      )}
    </section>
  );
}
