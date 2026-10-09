// SPDX-License-Identifier: Apache-2.0
import type { DiagramTableDto, SchemaDiagramDto } from '@openrows/ipc';

/** A table whose first column is the primary key `id`. */
export function table(name: string, ...columns: string[]): DiagramTableDto {
  return {
    name,
    columns: ['id', ...columns].map((column, index) => ({
      name: column,
      dataType: index ? 'integer' : 'bigint',
      nullable: index > 0,
      primaryKey: index === 0,
    })),
  };
}

/** customers ← orders ← lines, plus unrelated `audit` and `settings`. */
export const shop: SchemaDiagramDto = {
  tables: [
    table('audit', 'event'),
    table('customers', 'email'),
    table('lines', 'order_id', 'qty'),
    table('orders', 'customer_id'),
    table('settings', 'value'),
  ],
  foreignKeys: [
    {
      name: 'lines_order_fk',
      table: 'lines',
      columns: ['order_id'],
      targetTable: 'orders',
      targetColumns: ['id'],
    },
    {
      name: 'orders_customer_fk',
      table: 'orders',
      columns: ['customer_id'],
      targetTable: 'customers',
      targetColumns: ['id'],
    },
  ],
  truncated: false,
  limit: 400,
};
