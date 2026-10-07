// SPDX-License-Identifier: Apache-2.0
import { useRef, useState } from 'react';
import type { ReactNode } from 'react';
import {
  GlassPanel,
  IconButton,
  SegmentedControl,
  TextField,
  TreeItem,
  HighlightText,
} from '@openrows/ui';
import type { Database, DatabaseObject } from './data/databases';
import { savedQueries, history } from './data/connections';
import { sidebarSegments } from './data/content';
import { SidebarObjects, substringIndices } from './SidebarObjects';
import { SidebarHeader } from './SidebarHeader';
import styles from './Sidebar.module.css';
type Props = {
  database: Database;
  header: ReactNode;
  opened: readonly string[];
  onDatabase: (name: string) => void;
  onCloseDatabase: (name: string) => void;
  onSwitcher: () => void;
  table: string;
  schema: string;
  onSchema: (schema: string) => void;
  onTable: (object: DatabaseObject) => void;
  onQuery: () => void;
};
/** Live object search scoped to the active database and schema. */
export function WorkspaceSidebar(props: Props): React.JSX.Element {
  const [segment, setSegment] = useState('Items');
  const [search, setSearch] = useState('');
  const ref = useRef<HTMLDivElement>(null);
  const names = (segment === 'Queries' ? savedQueries : history).filter(
    (name) => name.toLowerCase().includes(search.toLowerCase()),
  );
  return (
    <div
      ref={ref}
      data-sidebar
      onKeyDown={(event) => {
        if (event.key === 'Escape') {
          event.preventDefault();
          setSearch('');
        }
        if (
          event.metaKey &&
          event.key.toLowerCase() === 'f' &&
          !event.altKey &&
          !event.ctrlKey &&
          !event.shiftKey &&
          !event.nativeEvent.isComposing
        ) {
          event.preventDefault();
          ref.current?.querySelector('input')?.focus();
        }
      }}
      className={styles.container}
    >
      <GlassPanel className={styles.sidebar}>
        <SidebarHeader {...props} database={props.database.name}>
          {props.header}
        </SidebarHeader>
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
            <SidebarObjects {...props} search={search} />
          ) : (
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
                    indices={substringIndices(name, search)}
                  />
                </TreeItem>
              ))}
              {!names.length && <p role="status">No matches</p>}
            </>
          )}
        </div>
        <SchemaPicker {...props} />
      </GlassPanel>
    </div>
  );
}

function SchemaPicker(props: Props): React.JSX.Element {
  return (
    <div className={styles.schema}>
      <IconButton aria-label="Create object" size="sm">
        +
      </IconButton>
      <select
        aria-label="Schema"
        value={props.schema}
        onChange={(event) => {
          props.onSchema(event.target.value);
        }}
      >
        {props.database.schemas.map((schema) => (
          <option key={schema}>{schema}</option>
        ))}
      </select>
    </div>
  );
}
