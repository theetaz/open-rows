// SPDX-License-Identifier: Apache-2.0
import { Icon, IconButton } from '@openrows/ui';
import type { ProfileDto, SavedQueryDto } from '@openrows/ipc';
import { SavedQueryList } from '../query';
import { moveToProject } from '../query/savedQueries';
import { ProjectSection, useProject } from '../projects';
import { reloadProject } from '../projects/projectStore';
import styles from './Sidebar.module.css';
import { t as shell } from '../../shell/messages';
import { t } from './messages';

/** What the Queries segment opens and reports. */
export type SidebarQueryProps = {
  /** Open a saved query of the active connection in a new query tab. */
  onSavedQuery: (query: SavedQueryDto) => void;
  /** Open a project query file (absolute path) in a tab bound to it. */
  onProjectQuery: (path: string) => void;
  /** A project file or folder moved; bound tabs follow. */
  onProjectMoved: (from: string, to: string) => void;
  /** Select a connection (the project's default one). */
  onConnection: (id: string) => void;
};

/**
 * The Queries segment: the open project's folder tree (or Open Project…),
 * then the connection's local, store-backed queries.
 */
export function SidebarQueries(
  props: SidebarQueryProps & {
    search: string;
    active: ProfileDto | undefined;
    profiles: readonly ProfileDto[];
  },
): React.JSX.Element {
  const { project } = useProject();
  return (
    <>
      <ProjectSection
        search={props.search}
        active={props.active}
        profiles={props.profiles}
        onConnection={props.onConnection}
        onOpenQuery={props.onProjectQuery}
        onMoved={props.onProjectMoved}
      />
      <section aria-label={t('localQueries')}>
        <h3 className={styles.sectionTitle}>{t('local')}</h3>
        <SavedQueryList
          profileId={props.active?.id}
          search={props.search}
          onOpen={props.onSavedQuery}
          onMoveToProject={
            project
              ? async (query) => {
                  await moveToProject(query, project);
                  await reloadProject();
                }
              : undefined
          }
        />
      </section>
    </>
  );
}

/**
 * The connection's saved queries under the object tree (Items), headed
 * like the tree with a `+` for a new query.
 */
export function SavedQueriesSection(props: {
  search: string;
  active: ProfileDto | undefined;
  onSavedQuery: (query: SavedQueryDto) => void;
  onNew: () => void;
}): React.JSX.Element | null {
  if (!props.active) return null;
  return (
    <section aria-label={shell('savedQueries')} className={styles.saved}>
      <h3 className={styles.treeHead}>
        <span>{shell('savedQueries')}</span>
        <IconButton
          aria-label={shell('newQuery')}
          title={shell('newQuery')}
          size="sm"
          variant="ghost"
          onClick={props.onNew}
        >
          <Icon name="plus" />
        </IconButton>
      </h3>
      <SavedQueryList
        profileId={props.active.id}
        search={props.search}
        onOpen={props.onSavedQuery}
      />
    </section>
  );
}
