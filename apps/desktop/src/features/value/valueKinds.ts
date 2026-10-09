// SPDX-License-Identifier: Apache-2.0
/** Image types the inspector previews, found from magic bytes rather than the column type. */
export type ImageMime = 'image/png' | 'image/jpeg' | 'image/gif' | 'image/webp';

const PNG = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
const JPEG = [0xff, 0xd8, 0xff];

function matchesAt(
  bytes: Uint8Array,
  offset: number,
  expected: readonly number[],
): boolean {
  return expected.every((byte, index) => bytes[offset + index] === byte);
}

function asciiAt(bytes: Uint8Array, offset: number, text: string): boolean {
  return matchesAt(
    bytes,
    offset,
    Array.from(text, (char) => char.charCodeAt(0)),
  );
}

/** The image type named by the leading bytes, or `null` when it is not PNG, JPEG, GIF or WebP. */
export function imageMime(bytes: Uint8Array): ImageMime | null {
  if (matchesAt(bytes, 0, PNG)) return 'image/png';
  if (matchesAt(bytes, 0, JPEG)) return 'image/jpeg';
  if (asciiAt(bytes, 0, 'GIF87a') || asciiAt(bytes, 0, 'GIF89a'))
    return 'image/gif';
  if (asciiAt(bytes, 0, 'RIFF') && asciiAt(bytes, 8, 'WEBP'))
    return 'image/webp';
  return null;
}

/**
 * The parsed document when `text` is a JSON object or array. Scalars such as
 * `42` or `"x"` stay text: a tree of one value is not worth a view.
 */
export function jsonContainer(text: string): object | null {
  const start = text.trimStart().charAt(0);
  if (start !== '{' && start !== '[') return null;
  try {
    const value: unknown = JSON.parse(text);
    return value !== null && typeof value === 'object' ? value : null;
  } catch {
    return null;
  }
}
