// SPDX-License-Identifier: Apache-2.0
/** Sample catalog shown until the real schema tree lands; not a connection. */
export const sampleCatalog: {
  database: string;
  schemas: readonly string[];
  tables: readonly (readonly [string, string])[];
} = {
  database: 'orders',
  schemas: ['public', 'archive'],
  tables: [
    ['orders', '2.4M'],
    ['order_items', '9.1M'],
    ['customers', '412K'],
    ['products', '3.2K'],
    ['payments', '2.3M'],
    ['shipments', '1.9M'],
  ],
};
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
