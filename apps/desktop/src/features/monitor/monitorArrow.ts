// SPDX-License-Identifier: Apache-2.0
import { tableFromIPC } from 'apache-arrow';

/** A bounded Arrow table, retaining validity and unit metadata. */
export type MonitorTable = {
  columns: { name: string; unit: string }[];
  rows: Record<string, string | number | null>[];
  state: string;
  partial: boolean;
};
/** Decode bounded Arrow fields and validity without coercing missing numbers to zero. */
export function decodeTable(bytes: ArrayBuffer): MonitorTable {
  const table = tableFromIPC(bytes);
  const columns = table.schema.fields.map((field) => ({
    name: field.name,
    unit: field.metadata.get('unit') ?? '',
  }));
  const rows: MonitorTable['rows'] = [];
  for (let index = 0; index < table.numRows; index++) {
    const row: MonitorTable['rows'][number] = {};
    for (const { name } of columns) {
      const value: unknown = table.getChild(name)?.get(index);
      row[name] =
        typeof value === 'string' ||
        (typeof value === 'number' && Number.isFinite(value))
          ? value
          : null;
    }
    rows.push(row);
  }
  return {
    columns,
    rows,
    state: table.schema.metadata.get('sampleState') ?? 'ok',
    partial: table.schema.metadata.get('partial') === 'true',
  };
}
