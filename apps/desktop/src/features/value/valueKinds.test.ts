// SPDX-License-Identifier: Apache-2.0
import { describe, expect, it } from 'vitest';
import { imageMime, jsonContainer } from './valueKinds';
import { hexDump } from './hexDump';
import { clipText, formatBytes } from './valueFormat';

const ascii = (text: string): Uint8Array =>
  Uint8Array.from(Array.from(text, (char) => char.charCodeAt(0)));

describe('imageMime', () => {
  it('recognises PNG, JPEG, GIF and WebP by their leading bytes', () => {
    expect(
      imageMime(
        Uint8Array.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0]),
      ),
    ).toBe('image/png');
    expect(imageMime(Uint8Array.from([0xff, 0xd8, 0xff, 0xe0]))).toBe(
      'image/jpeg',
    );
    expect(imageMime(ascii('GIF89a....'))).toBe('image/gif');
    expect(imageMime(ascii('RIFF\0\0\0\0WEBPVP8 '))).toBe('image/webp');
  });

  it('returns null for other bytes, including a RIFF file that is not WebP', () => {
    expect(imageMime(ascii('%PDF-1.7'))).toBeNull();
    expect(imageMime(ascii('RIFF\0\0\0\0WAVEfmt '))).toBeNull();
    expect(imageMime(new Uint8Array())).toBeNull();
  });
});

describe('jsonContainer', () => {
  it('parses objects and arrays only', () => {
    expect(jsonContainer(' {"a": [1, 2]} ')).toEqual({ a: [1, 2] });
    expect(jsonContainer('[1, "x"]')).toEqual([1, 'x']);
    expect(jsonContainer('42')).toBeNull();
    expect(jsonContainer('"text"')).toBeNull();
    expect(jsonContainer('{not json')).toBeNull();
  });
});

describe('hexDump', () => {
  it('prints offset, hex and ASCII columns for each sixteen bytes', () => {
    const dump = hexDump(ascii('Hello, OpenRows!'), 64);
    expect(dump.shown).toBe(16);
    expect(dump.text).toBe(
      '00000000  48 65 6c 6c 6f 2c 20 4f  70 65 6e 52 6f 77 73 21  |Hello, OpenRows!|',
    );
  });

  it('stops at the limit and pads a short last line', () => {
    const dump = hexDump(Uint8Array.from([0, 0x41, 0xff]), 2);
    expect(dump.shown).toBe(2);
    expect(dump.text.startsWith('00000000  00 41 ')).toBe(true);
    expect(dump.text.endsWith(`|.A${' '.repeat(14)}|`)).toBe(true);
    expect(dump.text.split('\n')).toHaveLength(1);
  });
});

describe('formatBytes and clipText', () => {
  it('formats sizes with binary units', () => {
    expect(formatBytes(812)).toBe('812 B');
    expect(formatBytes(1536)).toBe('1.5 KB');
    expect(formatBytes(20 * 1024 * 1024)).toBe('20 MB');
  });

  it('cuts text only when it is longer than the limit', () => {
    expect(clipText('abcdef', 4)).toEqual({ text: 'abcd', clipped: true });
    expect(clipText('abc', 4)).toEqual({ text: 'abc', clipped: false });
  });
});
