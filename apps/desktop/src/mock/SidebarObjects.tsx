// SPDX-License-Identifier: Apache-2.0
import { DisclosureSection, TreeItem, HighlightText } from '@openrows/ui';
import type { Database, DatabaseObject } from './data/databases';
/** All substring matches, including repeated occurrences, for sidebar highlighting. */
export function substringIndices(text: string, query: string): number[] {
  if (!query) return [];
  const source = text.toLowerCase();
  const needle = query.toLowerCase();
  const indices: number[] = [];
  let index = source.indexOf(needle);
  while (index >= 0) {
    for (let offset = 0; offset < needle.length; offset++)
      indices.push(index + offset);
    index = source.indexOf(needle, index + needle.length);
  }
  return indices;
}
/** Search forces matching sections open while preserving their disclosure state. */
export function SidebarObjects(props: {
  database: Database;
  schema: string;
  search: string;
  table: string;
  onTable: (object: DatabaseObject) => void;
}): React.JSX.Element {
  const objects = props.database.objects.filter(
    (object) => object.schema === props.schema,
  );
  const matches = objects.filter((object) =>
    object.name.toLowerCase().includes(props.search.toLowerCase()),
  );
  const kinds: DatabaseObject['kind'][] = [
    'Tables',
    'Views',
    'Functions',
    'Triggers',
    'Sequences',
  ];
  return (
    <>
      {!matches.length && <p role="status">No matches</p>}
      {kinds.map((kind) => {
        const items = matches.filter((object) => object.kind === kind);
        return (
          <div key={kind} hidden={!!props.search && !items.length}>
            <DisclosureSection
              label={kind}
              count={items.length}
              defaultOpen={kind === 'Tables'}
              forceOpen={!!props.search}
            >
              {items.map((object) => (
                <TreeItem
                  key={object.name}
                  label={object.name}
                  {...(object.count ? { count: object.count } : {})}
                  selected={object.name === props.table}
                  onSelect={() => {
                    props.onTable(object);
                  }}
                >
                  <HighlightText
                    text={object.name}
                    indices={substringIndices(object.name, props.search)}
                  />
                </TreeItem>
              ))}
            </DisclosureSection>
          </div>
        );
      })}
    </>
  );
}
