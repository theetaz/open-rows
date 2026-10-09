// SPDX-License-Identifier: Apache-2.0
import { useRef, useState } from 'react';
import type { ReactNode } from 'react';
import {
  GlassPanel,
  Icon,
  SegmentedControl,
  TextField,
  TreeItem,
  HighlightText,
} from '@openrows/ui';
import { ObjectTree, SchemaPicker, substringIndices } from '../features/schema';
import type { Catalog, ObjectRef } from '../features/schema';
import { HistoryList } from '../features/query';
import type { Connections } from '../features/connections';
import { savedQueries } from './data/connections';
import { sidebarSegments } from './data/content';
import styles from './Sidebar.module.css';
type Props = {
  catalog: Catalog;
  header: ReactNode;
  /** The saved connections; the History segment lists their statements. */
  connections: Connections;
  onOpen: (object: ObjectRef) => void;
  onQuery: () => void;
  /** Open a history statement in a new query tab. */
  onOpenSql: (sql: string) => void;
};
const exact = (event: React.KeyboardEvent, key: string): boolean =>
  event.metaKey &&
  event.key.toLowerCase() === key &&
  !event.altKey &&
  !event.ctrlKey &&
  !event.shiftKey &&
  !event.nativeEvent.isComposing;
/** Real object tree of the active database; sample saved queries and history. */
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
            segment === 'History' ? 'Search history…' : 'Search tables, views…'
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
            <SampleList
              names={savedQueries}
              search={search}
              onQuery={props.onQuery}
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

function SampleList(props: {
  names: readonly string[];
  search: string;
  onQuery: () => void;
}): React.JSX.Element {
  const names = props.names.filter((name) =>
    name.toLowerCase().includes(props.search.toLowerCase()),
  );
  return (
    <>
      {names.map((name) => (
        <TreeItem
          key={name}
          label={name}
          selected={false}
          onSelect={props.onQuery}
        >
          <HighlightText
            text={name}
            indices={substringIndices(name, props.search)}
          />
        </TreeItem>
      ))}
      {!names.length && <p role="status">No matches</p>}
    </>
  );
}
