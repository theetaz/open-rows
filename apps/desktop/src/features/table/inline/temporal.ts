// SPDX-License-Identifier: Apache-2.0

/** Which native picker a column type gets: a date, a time of day or both. */
export type TemporalKind = 'date' | 'time' | 'datetime';

/** The `<input type>` of each picker. */
export const NATIVE_TYPE: Readonly<Record<TemporalKind, string>> = {
  date: 'date',
  time: 'time',
  datetime: 'datetime-local',
};

/** The picker for a database type (date, time, timestamp, datetime…), or `null`. */
export function temporalKind(dataType: string): TemporalKind | null {
  const type = dataType
    .toLowerCase()
    .replace(/\(\d+\)/g, '')
    .trim();
  if (type === 'date') return 'date';
  if (/^time(tz| with(out)? time zone)?$/.test(type)) return 'time';
  if (/^(timestamp|datetime|smalldatetime|datetime2|datetimeoffset)/.test(type))
    return 'datetime';
  return null;
}

const DATE = /^(\d{4}-\d{2}-\d{2})/;
const TIME = /^(\d{2}:\d{2})(:\d{2}(?:\.\d+)?)?/;
/** A trailing zone offset (`+05:30`, `-08`, `Z`) of timetz/timestamptz text. */
const ZONE = /^(?:Z|[+-]\d{2}(?::?\d{2})?(?::?\d{2})?)$/;

function clockText(text: string, kind: TemporalKind): string {
  const clock = kind === 'time' ? text : text.slice(10);
  return clock.replace(/^[T\s]+/, '');
}

/** Offsets follow a clock; a negative MySQL TIME duration is not a zone. */
function zoneOf(previous: string, kind: TemporalKind): string {
  const text = clockText(previous, kind);
  const clock = TIME.exec(text)?.[0];
  return clock ? (ZONE.exec(text.slice(clock.length).trim())?.[0] ?? '') : '';
}

/** Cell text as the picker's value; empty when the text is not a date/time. */
export function toNative(text: string, kind: TemporalKind): string {
  const value = text.trim();
  const date = DATE.exec(value)?.[1];
  const time = TIME.exec(clockText(value, kind));
  const clock = time ? nativeClock(`${time[1] ?? ''}${time[2] ?? ':00'}`) : '';
  if (kind === 'date') return date ?? '';
  if (kind === 'time') return clock;
  return date ? `${date}T${clock || '00:00:00'}` : '';
}

/** Native controls support milliseconds; exact database precision stays in typed text. */
function nativeClock(clock: string): string {
  const [seconds = '', fraction] = clock.split('.');
  const milliseconds = fraction?.slice(0, 3).padEnd(3, '0');
  return milliseconds && milliseconds !== '000'
    ? `${seconds}.${milliseconds}`
    : seconds;
}

/** Keep sub-millisecond precision when only the date changes. */
function exactClock(
  clock: string,
  previous: string,
  kind: TemporalKind,
): string {
  const original = TIME.exec(clockText(previous, kind))?.[0];
  return original && nativeClock(original) === nativeClock(clock)
    ? original
    : clock;
}

/**
 * The picked value as SQL-friendly text (`2024-05-01 13:45:00`), keeping
 * the zone offset of `previous` for zoned columns; an emptied picker keeps
 * `previous`.
 */
export function fromNative(
  picked: string,
  previous: string,
  kind: TemporalKind,
): string {
  if (!picked || toNative(picked, kind) === toNative(previous, kind))
    return previous;
  if (kind === 'date') return picked;
  const before = previous.trim();
  const zone = zoneOf(before, kind);
  const [date, clock = picked] = kind === 'time' ? [''] : picked.split('T');
  const seconds = exactClock(
    /^\d{2}:\d{2}$/.test(clock) ? `${clock}:00` : clock,
    before,
    kind,
  );
  return `${date ? `${date} ` : ''}${seconds}${zone}`;
}
