// SPDX-License-Identifier: Apache-2.0
import { useCallback, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { GlassPanel, Icon, SegmentedControl, TextField } from '@openrows/ui';
import { ObjectTree, SchemaPicker } from '../schema';
import type { Catalog, MenuPoint, ObjectRef } from '../schema';
import { HistoryList, SavedQueryList } from '../query';
import type { SavedQueryDto } from '@openrows/ipc';
import type { Connections } from '../connections';
import { ObjectActions } from './ObjectActions';
import type { ObjectMenu, SessionLookup } from './ObjectActions';
import styles from './Sidebar.module.css';

/** The sidebar's views: catalog objects, saved queries and history. */
const sidebarSegments = ['Items', 'Queries', 'History'].map((label) => ({
  value: label,
  label,
}));
type Props = {
  catalog: Catalog;
  header: ReactNode;
  /** Engine tools beside the schema picker. */
  schemaTools?: ReactNode;
  /** The saved connections; the History segment lists their statements. */
  connections: Connections;
  /** Open an object; `preview` for a single click in the tree. */
  onOpen: (object: ObjectRef, preview?: boolean) => void;
  /** Open a history statement in a new query tab. */
  onOpenSql: (sql: string) => void;
  /** Open a saved query of the active connection in a new query tab. */
  onSavedQuery: (query: SavedQueryDto) => void;
  /** Sessions for exporting a table from its context menu. */
  sessionFor: SessionLookup;
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
  const [menu, setMenu] = useState<ObjectMenu | null>(null);
  const closeMenu = useCallback(() => {
    setMenu(null);
  }, []);
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
        <SidebarItems
          {...props}
          segment={segment}
          search={search}
          onMenu={(object, point) => {
            setMenu({ object, point });
          }}
        />
        {catalog.database && (
          <SchemaPicker catalog={catalog}>{props.schemaTools}</SchemaPicker>
        )}
      </GlassPanel>
      <ObjectActions
        menu={menu}
        onClose={closeMenu}
        sessionFor={props.sessionFor}
        onOpen={props.onOpen}
      />
    </div>
  );
}

/** The chosen sidebar segment: object tree, saved queries or history. */
function SidebarItems(
  props: Props & {
    segment: string;
    search: string;
    onMenu: (object: ObjectRef, point: MenuPoint) => void;
  },
): React.JSX.Element {
  return (
    <div className={styles.objects}>
      {props.segment === 'Items' && (
        <ObjectTree
          catalog={props.catalog}
          search={props.search}
          onOpen={props.onOpen}
          onMenu={props.onMenu}
        />
      )}
      {props.segment === 'Queries' && (
        <SavedQueryList
          profileId={props.connections.active?.id}
          search={props.search}
          onOpen={props.onSavedQuery}
        />
      )}
      {props.segment === 'History' && (
        <HistoryList
          active={props.connections.active}
          connections={props.connections.profiles}
          text={props.search}
          onOpen={props.onOpenSql}
        />
      )}
    </div>
  );
}
