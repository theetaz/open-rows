// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { fixedValue, intervalText } from './format';
import type { ColumnChunk } from './tiles';

function chunk(values: ArrayBufferView, stride = 1): ColumnChunk {
  return {
    length: 1,
    offset: 0,
    stride,
    nullCount: 0,
    values,
    validity: undefined,
    offsets: undefined,
  };
}

function decimal128(value: bigint): Uint32Array {
  const lanes = new Uint32Array(4);
  let bits = BigInt.asUintN(128, value);
  for (let lane = 0; lane < 4; lane++) {
    lanes[lane] = Number(bits & 0xffff_ffffn);
    bits >>= 32n;
  }
  return lanes;
}

it('formats decimals exactly for any scale and sign', () => {
  const text = (value: bigint, type: string): string =>
    fixedValue(chunk(decimal128(value), 4), type, 0);
  expect(text(12345n, 'Decimal[10e+2]')).toBe('123.45');
  expect(text(-5n, 'Decimal[10e+2]')).toBe('-0.05');
  expect(text(7n, 'Decimal[10e+0]')).toBe('7');
  expect(
    text(-170141183460469231731687303715884105728n, 'Decimal[38e+0]'),
  ).toBe('-170141183460469231731687303715884105728');
});

it('formats PostgreSQL temporal values in UTC with full precision', () => {
  const micros = BigInt64Array.of(1_700_000_000_123_456n);
  expect(fixedValue(chunk(micros), 'Timestamp<MICROSECOND>', 0)).toBe(
    '2023-11-14 22:13:20.123456',
  );
  expect(fixedValue(chunk(micros), 'Timestamp<MICROSECOND, UTC>', 0)).toBe(
    '2023-11-14 22:13:20.123456+00',
  );
  const before = BigInt64Array.of(-1n);
  expect(fixedValue(chunk(before), 'Timestamp<MICROSECOND>', 0)).toBe(
    '1969-12-31 23:59:59.999999',
  );
  const whole = BigInt64Array.of(86_400_000_000n);
  expect(fixedValue(chunk(whole), 'Timestamp<MICROSECOND>', 0)).toBe(
    '1970-01-02 00:00:00',
  );
  expect(fixedValue(chunk(Int32Array.of(19_675)), 'Date32<DAY>', 0)).toBe(
    '2023-11-14',
  );
});

it('pads fractions to the source precision so MySQL text matches exactly', () => {
  const micros = BigInt64Array.of(1_714_979_289_500_000n);
  const type = 'Timestamp<MICROSECOND>';
  expect(fixedValue(chunk(micros), type, 0, 3)).toBe('2024-05-06 07:08:09.500');
  expect(fixedValue(chunk(micros), type, 0, 0)).toBe('2024-05-06 07:08:09');
  expect(fixedValue(chunk(micros), type, 0)).toBe('2024-05-06 07:08:09.5');
  const old = BigInt64Array.of(-62_135_596_799_999_999n);
  expect(fixedValue(chunk(old), type, 0, 6)).toBe('0001-01-01 00:00:00.000001');
});

it('formats times of day with and without a fixed precision', () => {
  const time = BigInt64Array.of(86_399_999_000n);
  expect(fixedValue(chunk(time), 'Time64<MICROSECOND>', 0)).toBe(
    '23:59:59.999',
  );
  expect(fixedValue(chunk(time), 'Time64<MICROSECOND>', 0, 6)).toBe(
    '23:59:59.999000',
  );
  expect(
    fixedValue(
      chunk(BigInt64Array.of(3_723_000_000n)),
      'Time64<MICROSECOND>',
      0,
    ),
  ).toBe('01:02:03');
  expect(fixedValue(chunk(Int32Array.of(45_296)), 'Time32<SECOND>', 0)).toBe(
    '12:34:56',
  );
});

it('formats unsigned 64-bit integers above the signed range', () => {
  const max = BigUint64Array.of(18_446_744_073_709_551_615n);
  expect(fixedValue(chunk(max), 'Uint64', 0)).toBe('18446744073709551615');
});

it('formats integers, floats, UUIDs and fixed binaries', () => {
  expect(
    fixedValue(chunk(BigInt64Array.of(9_007_199_254_740_993n)), 'Int64', 0),
  ).toBe('9007199254740993');
  expect(fixedValue(chunk(Int16Array.of(-3)), 'Int16', 0)).toBe('-3');
  expect(fixedValue(chunk(Float32Array.of(0.1)), 'Float32', 0)).toBe('0.1');
  expect(fixedValue(chunk(Float64Array.of(2.5)), 'Float64', 0)).toBe('2.5');
  const uuid = Uint8Array.from({ length: 16 }, (_, index) => index * 17);
  expect(fixedValue(chunk(uuid, 16), 'FixedSizeBinary[16]', 0)).toBe(
    '00112233-4455-6677-8899-aabbccddeeff',
  );
  expect(
    fixedValue(chunk(Uint8Array.of(1, 255), 2), 'FixedSizeBinary[2]', 0),
  ).toBe('\\x01ff');
});

it('formats intervals like PostgreSQL IntervalStyle postgres', () => {
  const value = (months: number, days: number, nanos: bigint): string => {
    const bytes = new DataView(new ArrayBuffer(16));
    bytes.setInt32(0, months, true);
    bytes.setInt32(4, days, true);
    bytes.setBigInt64(8, nanos, true);
    return fixedValue(
      chunk(new Uint8Array(bytes.buffer), 16),
      'Interval<MONTH_DAY_NANO>',
      0,
    );
  };
  expect(value(14, 3, 14_706_500_000_000n)).toBe(
    '1 year 2 mons 3 days 04:05:06.5',
  );
  expect(value(0, 0, 0n)).toBe('00:00:00');
  expect(value(0, -1, 3_600_000_000_000n)).toBe('-1 days +01:00:00');
  expect(value(-14, 3, -1_000_000_000n)).toBe(
    '-1 years -2 mons +3 days -00:00:01',
  );
  expect(intervalText(0, 1, 0n)).toBe('1 day');
});
