// SPDX-License-Identifier: Apache-2.0
import type { ProfileDto, SavedQueryDto } from '@openrows/ipc';
import { SavedQueryList } from '../query';
import { moveToProject } from '../query/savedQueries';
import { ProjectSection, useProject } from '../projects';
import { reloadProject } from '../projects/projectStore';
import styles from './Sidebar.module.css';

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
      <section aria-label="Local queries">
        <h3 className={styles.sectionTitle}>Local</h3>
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
