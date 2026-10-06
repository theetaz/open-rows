// SPDX-License-Identifier: Apache-2.0
export type WorkspaceTab = {
  id: string;
  label: string;
  kind: 'table' | 'query';
  sql?: string;
};
export const initialTabs: readonly WorkspaceTab[] = [
  { id: 'orders', label: 'orders', kind: 'table' },
  { id: 'customers', label: 'customers', kind: 'table' },
  { id: 'weekly', label: 'weekly revenue.sql', kind: 'query' },
  { id: 'query', label: 'SQL Query', kind: 'query' },
];
export const sidebarSegments = ['Items', 'Queries', 'History'].map((label) => ({
  value: label,
  label,
}));
export const tableSegments = [
  'Data',
  'Structure',
  'Indexes',
  'Triggers',
  'DDL',
].map((label) => ({ value: label, label }));
export const inspectorSegments = ['Details', 'Assistant'].map((label) => ({
  value: label,
  label,
}));
export const reviewSegments = ['Changes', 'SQL'].map((label) => ({
  value: label,
  label,
}));
export const structures = [
  ['id', 'bigint', 'Primary key', 'nextval(orders_id_seq)'],
  ['customer_id', 'bigint', 'NOT NULL', '—'],
  ['status', 'text', 'NOT NULL', "'pending'"],
  ['total', 'numeric(12,2)', 'NOT NULL', '0.00'],
  ['currency', 'char(3)', 'NOT NULL', "'USD'"],
  ['created_at', 'timestamptz', 'NOT NULL', 'now()'],
  ['notes', 'text', 'Nullable', 'NULL'],
];
export const indexes = [
  ['orders_pkey', 'btree', 'id', 'Unique'],
  ['orders_status_created_idx', 'btree', 'status, created_at', 'Non-unique'],
  ['orders_customer_idx', 'btree', 'customer_id', 'Non-unique'],
];
export const triggers = [
  ['orders_updated_at', 'BEFORE UPDATE', 'set_updated_at()'],
  ['audit_orders', 'AFTER INSERT OR UPDATE OR DELETE', 'audit_change()'],
];
export const assistantPrompt =
  'Pending orders older than 7 days, grouped by currency';
export const policyChips = [
  'Shares schema only',
  'Read-only tools',
  'MCP · 2 servers',
];
export const toolTrace = [
  'Read schema of public.orders',
  'Checked indexes on status, created_at',
];
export const assistantSql = `SELECT currency,
       count(*)   AS orders,
       sum(total) AS value
FROM orders
WHERE status = 'pending'
  AND created_at < now() - interval '7 days'
GROUP BY currency
ORDER BY value DESC;`;
export const ddl = `CREATE TABLE public.orders (
  id bigint PRIMARY KEY,
  customer_id bigint NOT NULL,
  status text NOT NULL DEFAULT 'pending',
  total numeric(12,2) NOT NULL DEFAULT 0,
  currency char(3) NOT NULL DEFAULT 'USD',
  created_at timestamptz NOT NULL DEFAULT now(),
  notes text
);
CREATE INDEX orders_status_created_idx
  ON public.orders (status, created_at);`;
export const changes = [
  'UPDATE · Order 10422 · total: 46.00 → 48.00',
  'UPDATE · Order 10424 · status: pending → shipped',
  'INSERT · New order · total: 59.00 USD',
  'DELETE · Order 10427 · Duplicate of 10422',
];
export const changeSql = `BEGIN;
UPDATE public.orders SET total = 48.00 WHERE id = 10422;
UPDATE public.orders SET status = 'shipped' WHERE id = 10424;
INSERT INTO public.orders (customer_id, total, currency, notes)
VALUES (5512, 59.00, 'USD', 'Manual re-order');
DELETE FROM public.orders WHERE id = 10427;
COMMIT;`;
