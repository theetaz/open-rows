// SPDX-License-Identifier: Apache-2.0
import { useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { GlassPanel, Icon, SegmentedControl, TextField } from '@openrows/ui';
import { ObjectTree, SchemaPicker } from '../features/schema';
import type { Catalog, ObjectRef } from '../features/schema';
import { HistoryList, SavedQueryList } from '../features/query';
import type { SavedQueryDto } from '@openrows/ipc';
import type { Connections } from '../features/connections';
import { sidebarSegments } from './data/content';
import styles from './Sidebar.module.css';
type Props = {
  catalog: Catalog;
  header: ReactNode;
  /** The saved connections; the History segment lists their statements. */
  connections: Connections;
  onOpen: (object: ObjectRef) => void;
  /** Open a history statement in a new query tab. */
  onOpenSql: (sql: string) => void;
  /** Open a saved query of the active connection in a new query tab. */
  onSavedQuery: (query: SavedQueryDto) => void;
};
const exact = (event: React.KeyboardEvent, key: string): boolean =>
  event.metaKey &&
  event.key.toLowerCase() === key &&
  !event.altKey &&
  !event.ctrlKey &&
  !event.shiftKey &&
  !event.nativeEvent.isComposing;
/** Object tree of the active database, saved queries and history. */
export function WorkspaceSidebar(props: Props): React.JSX.Element {
  const { catalog } = props;
  const [segment, setSegment] = useState('Items');
  const [search, setSearch] = useState('');
  const ref = useRef<HTMLDivElement>(null);
  return (
    <div
      ref={ref}
      data-sidebar
      onKeyDown={(event) => {
        if (event.key === 'Escape') {
          event.preventDefault();
          setSearch('');
        } else if (exact(event, 'f')) {
          event.preventDefault();
          ref.current?.querySelector('input')?.focus();
        } else if (exact(event, 'r')) {
          event.preventDefault();
          catalog.refresh();
        }
      }}
      className={styles.container}
    >
      <GlassPanel className={styles.sidebar}>
        <SegmentedControl
          aria-label="Sidebar mode"
          options={sidebarSegments}
          value={segment}
          onValueChange={setSegment}
          className={styles.modes}
        />
        {props.header}
        <TextField
          className={styles.search}
          leadingIcon={<Icon name="search" />}
          aria-label="Search items"
          placeholder={
            segment === 'History'
              ? 'Search history…'
              : segment === 'Queries'
                ? 'Search queries…'
                : 'Search tables, views…'
          }
          value={search}
          onChange={(event) => {
            setSearch(event.target.value);
          }}
        />
        <div className={styles.objects}>
          {segment === 'Items' && (
            <ObjectTree
              catalog={catalog}
              search={search}
              onOpen={props.onOpen}
            />
          )}
          {segment === 'Queries' && (
            <SavedQueryList
              profileId={props.connections.active?.id}
              search={search}
              onOpen={props.onSavedQuery}
            />
          )}
          {segment === 'History' && (
            <HistoryList
              active={props.connections.active}
              connections={props.connections.profiles}
              text={search}
              onOpen={props.onOpenSql}
            />
          )}
        </div>
        {catalog.database && <SchemaPicker catalog={catalog} />}
      </GlassPanel>
    </div>
  );
}
