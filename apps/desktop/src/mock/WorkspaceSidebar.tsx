// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import {
  DisclosureSection,
  GlassPanel,
  IconButton,
  SegmentedControl,
  TextField,
  TreeItem,
} from '@openrows/ui';
import type { Connection } from './data/connections';
import { objectSections, savedQueries, history } from './data/connections';
import { sidebarSegments } from './data/content';
import styles from './Sidebar.module.css';

type Props = {
  connection: Connection;
  table: string;
  schema: string;
  onSchema: (schema: string) => void;
  onTable: (table: string) => void;
  onQuery: () => void;
};
/** Glass object browser with independent segment, disclosure and search state. */
export function WorkspaceSidebar(props: Props): React.JSX.Element {
  const [segment, setSegment] = useState('Items');
  const [search, setSearch] = useState('');
  return (
    <GlassPanel className={styles.sidebar}>
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
          <Objects {...props} search={search} />
        ) : (
          (segment === 'Queries' ? savedQueries : history)
            .filter((name) => name.toLowerCase().includes(search.toLowerCase()))
            .map((name) => (
              <TreeItem
                key={name}
                label={name}
                selected={false}
                onSelect={props.onQuery}
              />
            ))
        )}
      </div>
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
          {props.connection.schemas.map((schema) => (
            <option key={schema}>{schema}</option>
          ))}
        </select>
      </div>
    </GlassPanel>
  );
}
function Objects(props: Props & { search: string }): React.JSX.Element {
  const includes = (name: string): boolean =>
    name.toLowerCase().includes(props.search.toLowerCase());
  return (
    <>
      <DisclosureSection
        label="Tables"
        count={props.connection.tables.length}
        defaultOpen
      >
        {props.connection.tables
          .filter(([name]) => includes(name))
          .map(([name, count]) => (
            <TreeItem
              key={name}
              label={name}
              count={count}
              selected={name === props.table}
              onSelect={() => {
                props.onTable(name);
              }}
            />
          ))}
      </DisclosureSection>
      {objectSections.map((section) => (
        <DisclosureSection
          key={section.name}
          label={section.name}
          count={section.items.length}
        >
          {section.items.filter(includes).map((name) => (
            <TreeItem
              key={name}
              label={name}
              selected={name === props.table}
              onSelect={() => {
                props.onTable(name);
              }}
            />
          ))}
        </DisclosureSection>
      ))}
    </>
  );
}
