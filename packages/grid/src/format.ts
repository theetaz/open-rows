// SPDX-License-Identifier: Apache-2.0
import type { ColumnChunk } from './tiles';

/** Display text of fixed-width values (Arrow JS type strings, e.g. `Decimal[10e+2]`). */
export function fixedValue(
  chunk: ColumnChunk,
  type: string,
  row: number,
): string {
  if (type.startsWith('Decimal')) return decimal(chunk, row, scaleOf(type));
  if (type.startsWith('Timestamp')) return timestamp(chunk, row, type);
  if (type.startsWith('Date32')) return date(chunk, row);
  if (type.startsWith('FixedSizeBinary')) return hex(chunk, row);
  if (type === 'Int64' || type === 'Uint64') return String(bigints(chunk)[row]);
  const value = (chunk.values as unknown as ArrayLike<number>)[row];
  if (value === undefined) return '';
  // Float32 values widen to float64 noise (0.1 → 0.10000000149…); 7 digits round-trip.
  return type === 'Float32'
    ? String(Number(value.toPrecision(7)))
    : String(value);
}

function bigints(chunk: ColumnChunk): BigInt64Array {
  return new BigInt64Array(
    chunk.values.buffer,
    chunk.values.byteOffset,
    chunk.values.byteLength / 8,
  );
}

function scaleOf(type: string): number {
  const match = /e([+-]?\d+)\]$/.exec(type);
  return match?.[1] ? Number(match[1]) : 0;
}

/** Exact Decimal128 text from four little-endian 32-bit lanes. */
function decimal(chunk: ColumnChunk, row: number, scale: number): string {
  const lanes = new Uint32Array(
    chunk.values.buffer,
    chunk.values.byteOffset,
    chunk.values.byteLength / 4,
  );
  let value = 0n;
  for (let lane = 3; lane >= 0; lane--)
    value = (value << 32n) | BigInt(lanes[row * 4 + lane] ?? 0);
  const signed = BigInt.asIntN(128, value);
  if (scale <= 0) return (signed * 10n ** BigInt(-scale)).toString();
  const digits = (signed < 0n ? -signed : signed)
    .toString()
    .padStart(scale + 1, '0');
  const sign = signed < 0n ? '-' : '';
  return `${sign}${digits.slice(0, -scale)}.${digits.slice(-scale)}`;
}

const UNIT_PER_SECOND: Record<string, bigint> = {
  SECOND: 1n,
  MILLISECOND: 1_000n,
  MICROSECOND: 1_000_000n,
  NANOSECOND: 1_000_000_000n,
};

/** `YYYY-MM-DD HH:MM:SS[.fraction]` in UTC; zoned columns append `+00`. */
function timestamp(chunk: ColumnChunk, row: number, type: string): string {
  const unit = /<(\w+)/.exec(type)?.[1] ?? 'MICROSECOND';
  const perSecond = UNIT_PER_SECOND[unit] ?? 1_000_000n;
  const value = bigints(chunk)[row] ?? 0n;
  let seconds = value / perSecond;
  let fraction = value % perSecond;
  if (fraction < 0n) {
    seconds -= 1n;
    fraction += perSecond;
  }
  const iso = new Date(Number(seconds) * 1000).toISOString();
  const width = String(perSecond).length - 1;
  const digits = fraction.toString().padStart(width, '0').replace(/0+$/, '');
  const zone = type.includes(',') ? '+00' : '';
  return `${iso.slice(0, 10)} ${iso.slice(11, 19)}${digits ? `.${digits}` : ''}${zone}`;
}

/** Days since the Unix epoch as `YYYY-MM-DD`. */
function date(chunk: ColumnChunk, row: number): string {
  const days = (chunk.values as Int32Array)[row] ?? 0;
  return new Date(days * 86_400_000).toISOString().slice(0, 10);
}

/** UUIDs (16 bytes) in canonical form; other fixed binaries as hex. */
function hex(chunk: ColumnChunk, row: number): string {
  const bytes = new Uint8Array(
    chunk.values.buffer,
    chunk.values.byteOffset + row * chunk.stride,
    chunk.stride,
  );
  const text = Array.from(bytes, (byte) =>
    byte.toString(16).padStart(2, '0'),
  ).join('');
  if (chunk.stride !== 16) return `\\x${text}`;
  return `${text.slice(0, 8)}-${text.slice(8, 12)}-${text.slice(12, 16)}-${text.slice(16, 20)}-${text.slice(20)}`;
}
