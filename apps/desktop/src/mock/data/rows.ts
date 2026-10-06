// SPDX-License-Identifier: Apache-2.0
export type OrderRow = readonly [
  number | string,
  number,
  'pending' | 'shipped' | 'paid' | 'cancelled',
  string,
  string,
  string,
  string | null,
];
export const columns = [
  'ID',
  'Customer',
  'Status',
  'Total',
  'Currency',
  'Created',
  'Notes',
];
const prototypeRows: readonly OrderRow[] = [
  [
    10421,
    5512,
    'pending',
    '120.50',
    'USD',
    'Sep 30, 10:12',
    'Leave at front desk',
  ],
  [10422, 1043, 'pending', '48.00', 'EUR', 'Sep 30, 10:15', null],
  [10423, 7781, 'pending', '9.99', 'USD', 'Sep 30, 10:20', 'Gift wrap'],
  [10424, 2290, 'shipped', '342.10', 'GBP', 'Sep 30, 10:22', null],
  [10425, 3307, 'pending', '75.25', 'USD', 'Sep 30, 10:31', null],
  [
    10426,
    9014,
    'pending',
    '18.40',
    'AUD',
    'Sep 30, 10:36',
    'Call before delivery',
  ],
  [
    10427,
    1043,
    'cancelled',
    '210.00',
    'EUR',
    'Sep 30, 10:41',
    'Duplicate of 10422',
  ],
  [10428, 6620, 'pending', '64.90', 'USD', 'Sep 30, 10:44', null],
  [
    10429,
    4471,
    'paid',
    '1,299.00',
    'USD',
    'Sep 30, 10:52',
    'B2B invoice INV-2291',
  ],
  [10430, 5512, 'pending', '33.00', 'USD', 'Sep 30, 11:03', null],
  [10431, 8183, 'pending', '89.95', 'CAD', 'Sep 30, 11:09', null],
  [10432, 2701, 'pending', '14.50', 'USD', 'Sep 30, 11:15', 'Fragile'],
  [10433, 3307, 'pending', '256.70', 'EUR', 'Sep 30, 11:21', null],
  ['New', 5512, 'pending', '59.00', 'USD', 'Default', 'Manual re-order'],
  [10434, 6105, 'pending', '42.00', 'USD', 'Sep 30, 11:30', null],
  [10435, 7781, 'pending', '7.25', 'USD', 'Sep 30, 11:34', null],
  [10436, 1190, 'pending', '118.00', 'JPY', 'Sep 30, 11:40', 'Express'],
];
export const rows: readonly OrderRow[] = [
  ...prototypeRows,
  ...Array.from({ length: 23 }, (_, index): OrderRow => [
    10437 + index,
    5512 + index * 17,
    'pending',
    (24.5 + index * 3).toFixed(2),
    'USD',
    `Sep 30, 12:${String(index + 10)}`,
    null,
  ]),
];
export const addedRow: OrderRow = [
  'New draft',
  5512,
  'pending',
  '0.00',
  'USD',
  'Default',
  null,
];
export function isEdited(row: number, column: number): boolean {
  return (row === 3 && column === 2) || (row === 1 && column === 3);
}
export const statusTones = {
  pending: 'warning',
  shipped: 'success',
  paid: 'info',
  cancelled: 'neutral',
} as const;

export function discardedRows(): readonly OrderRow[] {
  return rows
    .filter((row) => row[0] !== 'New')
    .map((row): OrderRow => {
      if (row[0] === 10422)
        return [row[0], row[1], row[2], '46.00', row[4], row[5], row[6]];
      if (row[0] === 10424)
        return [row[0], row[1], 'pending', row[3], row[4], row[5], row[6]];
      return row;
    });
}
