// SPDX-License-Identifier: Apache-2.0
/** Bytes per dump line, split in two groups of eight. */
const WIDTH = 16;
const HALF = 8;

/** The dump text and how many leading bytes it covers. */
type HexDump = { text: string; shown: number };

const printable = (byte: number): string =>
  byte >= 0x20 && byte <= 0x7e ? String.fromCharCode(byte) : '.';

function line(bytes: Uint8Array, start: number, end: number): string {
  const hex: string[] = [];
  let ascii = '';
  for (let index = start; index < start + WIDTH; index += 1) {
    const byte = bytes[index];
    if (index < end && byte !== undefined) {
      hex.push(byte.toString(16).padStart(2, '0'));
      ascii += printable(byte);
    } else {
      hex.push('  ');
      ascii += ' ';
    }
  }
  const offset = start.toString(16).padStart(8, '0');
  return `${offset}  ${hex.slice(0, HALF).join(' ')}  ${hex.slice(HALF).join(' ')}  |${ascii}|`;
}

/** `offset | hex | ASCII` lines for the first `limit` bytes of `bytes`. */
export function hexDump(bytes: Uint8Array, limit: number): HexDump {
  const shown = Math.min(bytes.length, limit);
  const lines: string[] = [];
  for (let start = 0; start < shown; start += WIDTH)
    lines.push(line(bytes, start, shown));
  return { text: lines.join('\n'), shown };
}
