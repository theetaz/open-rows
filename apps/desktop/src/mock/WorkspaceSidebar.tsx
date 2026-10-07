// SPDX-License-Identifier: Apache-2.0
import { useRef, useState } from 'react';
import type { ReactNode } from 'react';
import {
  GlassPanel,
  SegmentedControl,
  TextField,
  TreeItem,
  HighlightText,
} from '@openrows/ui';
import {
  DatabaseSelector,
  ObjectTree,
  SchemaPicker,
  substringIndices,
} from '../features/schema';
import type { Catalog, ObjectRef } from '../features/schema';
import { savedQueries, history } from './data/connections';
import { sidebarSegments } from './data/content';
import styles from './Sidebar.module.css';
type Props = {
  catalog: Catalog;
  header: ReactNode;
  onSwitcher: () => void;
  onOpen: (object: ObjectRef) => void;
  onQuery: () => void;
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
        <header className={styles.header}>
          {props.header}
          <DatabaseSelector
            database={catalog.database}
            connected={!catalog.offline}
            opened={catalog.opened}
            onDatabase={catalog.openDatabase}
            onCloseDatabase={catalog.closeDatabase}
            onSwitcher={props.onSwitcher}
          />
        </header>
        <SegmentedControl
          aria-label="Sidebar mode"
          options={sidebarSegments}
          value={segment}
          onValueChange={setSegment}
        />
        <TextField
          aria-label="Search items"
          placeholder="Search tables, views…"
          value={search}
          onChange={(event) => {
            setSearch(event.target.value);
          }}
        />
        <div className={styles.objects}>
          {segment === 'Items' ? (
            <ObjectTree
              catalog={catalog}
              search={search}
              onOpen={props.onOpen}
            />
          ) : (
            <SampleList
              names={segment === 'Queries' ? savedQueries : history}
              search={search}
              onQuery={props.onQuery}
            />
          )}
        </div>
        <SchemaPicker catalog={catalog} />
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
