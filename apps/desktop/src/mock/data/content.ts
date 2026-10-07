// SPDX-License-Identifier: Apache-2.0
export const sidebarSegments = ['Items', 'Queries', 'History'].map((label) => ({
  value: label,
  label,
}));
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
