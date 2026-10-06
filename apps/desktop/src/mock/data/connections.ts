// SPDX-License-Identifier: Apache-2.0
export type Connection = {
  name: string;
  database: string;
  tint: 'pg' | 'ch' | 'sqlite';
  host: string;
  environment: 'staging' | 'prod' | 'dev' | 'local';
  engine: string;
  route: string;
  tables: readonly (readonly [string, string])[];
  schemas: readonly string[];
};
export const connections: readonly Connection[] = [
  {
    name: 'orders-staging',
    database: 'orders',
    tint: 'pg',
    host: 'db.example.test',
    environment: 'staging',
    engine: 'PostgreSQL 16.4',
    route: 'SSH',
    schemas: ['public', 'archive'],
    tables: [
      ['orders', '2.4M'],
      ['order_items', '9.1M'],
      ['customers', '412K'],
      ['products', '3.2K'],
      ['payments', '2.3M'],
      ['shipments', '1.9M'],
    ],
  },
  {
    name: 'analytics-prod',
    database: 'analytics',
    tint: 'pg',
    host: 'db.example.test',
    environment: 'prod',
    engine: 'PostgreSQL 15.8',
    route: 'Cloud SQL proxy',
    schemas: ['public', 'reporting'],
    tables: [
      ['daily_revenue', '86K'],
      ['cohorts', '18K'],
      ['conversion_events', '8.2M'],
    ],
  },
  {
    name: 'events',
    database: 'default',
    tint: 'ch',
    host: 'events.example.test',
    environment: 'dev',
    engine: 'ClickHouse 24.8',
    route: 'HTTPS',
    schemas: ['default', 'system'],
    tables: [
      ['events', '12.8M'],
      ['sessions', '920K'],
      ['page_views', '4.1M'],
    ],
  },
  {
    name: 'app.sqlite',
    database: 'main',
    tint: 'sqlite',
    host: 'local file',
    environment: 'local',
    engine: 'SQLite 3.46',
    route: 'file',
    schemas: ['main', 'temp'],
    tables: [
      ['users', '128'],
      ['settings', '24'],
      ['notes', '640'],
    ],
  },
];
export const objectSections: readonly {
  name: 'Views' | 'Functions' | 'Triggers' | 'Sequences';
  items: readonly string[];
}[] = [
  {
    name: 'Views',
    items: ['order_summary', 'active_customers', 'daily_revenue'],
  },
  {
    name: 'Functions',
    items: ['calc_tax()', 'refresh_totals()', 'set_updated_at()'],
  },
  { name: 'Triggers', items: ['orders_updated_at', 'audit_orders'] },
  { name: 'Sequences', items: ['orders_id_seq', 'customers_id_seq'] },
];
export const savedQueries = [
  'weekly revenue.sql',
  'Pending orders by currency',
  'Top customers',
];
export const history = [
  'Today · SELECT * FROM orders · 38 ms',
  'Today · Weekly revenue · 22 ms',
  'Yesterday · Customers · 16 ms',
];
