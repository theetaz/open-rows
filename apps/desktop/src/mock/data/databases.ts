// SPDX-License-Identifier: Apache-2.0
import { objectSections, sampleCatalog } from './connections';
/** Hard-coded catalog objects; no server introspection. */
export type DatabaseObject = {
  name: string;
  schema: string;
  kind: 'Tables' | 'Views' | 'Functions' | 'Triggers' | 'Sequences';
  count?: string;
};
export type Database = {
  name: string;
  schemas: readonly string[];
  system?: boolean;
  objects: readonly DatabaseObject[];
};
const archive: Database = {
  name: 'orders_archive',
  schemas: ['public', 'history'],
  objects: [
    { name: 'orders_2024', schema: 'public', kind: 'Tables', count: '820K' },
    {
      name: 'archived_customers',
      schema: 'public',
      kind: 'Tables',
      count: '54K',
    },
    { name: 'annual_totals', schema: 'history', kind: 'Views', count: '12' },
    { name: 'restore_order()', schema: 'history', kind: 'Functions' },
  ],
};
const scratch: Database = {
  name: 'analytics_scratch',
  schemas: ['public', 'experiments'],
  objects: [
    { name: 'sample_events', schema: 'public', kind: 'Tables', count: '8K' },
    {
      name: 'trial_cohorts',
      schema: 'experiments',
      kind: 'Views',
      count: '24',
    },
  ],
};
const system = ['postgres', 'template1'].map((name): Database => ({
  name,
  schemas: ['public'],
  system: true,
  objects: [],
}));
const schema = sampleCatalog.schemas[0] ?? 'public';
const primary: Database = {
  name: sampleCatalog.database,
  schemas: sampleCatalog.schemas,
  objects: [
    ...sampleCatalog.tables.map(([name, count]): DatabaseObject => ({
      name,
      count,
      schema,
      kind: 'Tables',
    })),
    ...objectSections.flatMap((section) =>
      section.items.map((name): DatabaseObject => ({
        name,
        schema,
        kind: section.name,
      })),
    ),
    {
      name: 'session_notes',
      schema: sampleCatalog.schemas[1] ?? schema,
      kind: 'Tables',
      count: '16',
    },
  ],
};
/** The sample workspace catalog: primary, two extra and two system databases. */
export const databases: readonly Database[] = [
  primary,
  archive,
  scratch,
  ...system,
];
/** Validated catalog lookup; unknown names fall back to the primary database. */
export function databaseFor(name: string): Database {
  return databases.find((database) => database.name === name) ?? primary;
}
