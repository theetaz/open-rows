// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { fromNative, temporalKind, toNative } from './temporal';

it('gives date, time and timestamp columns their picker', () => {
  expect(temporalKind('date')).toBe('date');
  expect(temporalKind('time without time zone')).toBe('time');
  expect(temporalKind('timetz')).toBe('time');
  expect(temporalKind('timestamp(3) with time zone')).toBe('datetime');
  expect(temporalKind('timestamptz')).toBe('datetime');
  expect(temporalKind('DATETIME')).toBe('datetime');
  expect(temporalKind('datetime2(7)')).toBe('datetime');
  expect(temporalKind('text')).toBeNull();
  expect(temporalKind('interval')).toBeNull();
});

it('reads cell text into the native picker value', () => {
  expect(toNative('2024-05-01', 'date')).toBe('2024-05-01');
  expect(toNative('2024-05-01 13:45:10.123+02', 'datetime')).toBe(
    '2024-05-01T13:45:10.123',
  );
  expect(toNative('2024-05-01', 'datetime')).toBe('2024-05-01T00:00:00');
  expect(toNative('13:45', 'time')).toBe('13:45:00');
  expect(toNative('not a date', 'date')).toBe('');
});

it('writes picked values as SQL text, keeping zone offsets', () => {
  expect(fromNative('2024-06-02', '2024-05-01', 'date')).toBe('2024-06-02');
  expect(
    fromNative('2024-06-02T08:30', '2024-05-01 13:45:10+05:30', 'datetime'),
  ).toBe('2024-06-02 08:30:00+05:30');
  expect(fromNative('2024-06-02T08:30:15', '', 'datetime')).toBe(
    '2024-06-02 08:30:15',
  );
  expect(fromNative('2024-06-02T08:30', '2024-05-01', 'datetime')).toBe(
    '2024-06-02 08:30:00',
  );
  expect(fromNative('09:15', '13:45:00Z', 'time')).toBe('09:15:00Z');
  expect(fromNative('', '13:45:00', 'time')).toBe('13:45:00');
});

it('keeps negative MySQL durations in typed entry without treating them as zone offsets', () => {
  expect(toNative('-12:30:00', 'time')).toBe('');
  expect(fromNative('', '-12:30:00', 'time')).toBe('-12:30:00');
  expect(fromNative('09:15', '-12:30:00', 'time')).toBe('09:15:00');
  expect(fromNative('12:30', '-12:30:00', 'time')).toBe('12:30:00');
  expect(toNative('2024-05-01T13:45:10.123Z', 'datetime')).toBe(
    '2024-05-01T13:45:10.123',
  );
  expect(
    fromNative(
      '2024-06-02T13:45:10.123',
      '2024-05-01T13:45:10.123Z',
      'datetime',
    ),
  ).toBe('2024-06-02 13:45:10.123Z');
});

it('preserves sub-millisecond values when the native control omits zero milliseconds', () => {
  const original = '2024-05-01 13:45:00.000001+05:30';
  expect(toNative(original, 'datetime')).toBe('2024-05-01T13:45:00');
  expect(fromNative('2024-05-01T13:45', original, 'datetime')).toBe(original);
  expect(fromNative('2024-06-02T13:45', original, 'datetime')).toBe(
    '2024-06-02 13:45:00.000001+05:30',
  );
});

it('keeps exact fractional precision and offset when the picker value is unchanged', () => {
  const original = '2024-05-01 13:45:10.123456+05:30';
  expect(toNative(original, 'datetime')).toBe('2024-05-01T13:45:10.123');
  expect(fromNative('2024-05-01T13:45:10.123', original, 'datetime')).toBe(
    original,
  );
  expect(fromNative('2024-06-02T13:45:10.123', original, 'datetime')).toBe(
    '2024-06-02 13:45:10.123456+05:30',
  );
  expect(fromNative('09:15:02.25', '13:45:00+02', 'time')).toBe(
    '09:15:02.25+02',
  );
});
