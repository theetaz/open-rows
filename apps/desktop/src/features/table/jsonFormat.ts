// SPDX-License-Identifier: Apache-2.0

type Writer = { parts: string[]; depth: number; pretty: boolean };
const closers: Record<string, string> = { '{': '}', '[': ']' };
const blank = new Set([' ', '\t', '\n', '\r']);

/**
 * PRETTY (two-space indent) or MINIFY a JSON document, or `null` when it is
 * not valid JSON. Only insignificant whitespace changes: numbers keep their
 * exact digits and strings their exact escapes, unlike a parse/stringify trip.
 */
export function formatJson(text: string, pretty: boolean): string | null {
  try {
    JSON.parse(text);
  } catch {
    return null;
  }
  const writer: Writer = { parts: [], depth: 0, pretty };
  let index = 0;
  while (index < text.length) index = token(text, index, writer);
  return writer.parts.join('');
}

/** Emit the token at `index`; returns the index after it. */
function token(text: string, index: number, writer: Writer): number {
  const char = text.charAt(index);
  if (char === '"') {
    const end = stringEnd(text, index);
    writer.parts.push(text.slice(index, end));
    return end;
  }
  if (blank.has(char)) return index + 1;
  const closer = closers[char];
  if (closer) return open(text, index, closer, writer);
  if (char === '}' || char === ']') {
    writer.depth -= 1;
    writer.parts.push(newline(writer), char);
  } else if (char === ',') writer.parts.push(',', newline(writer));
  else if (char === ':') writer.parts.push(writer.pretty ? ': ' : ':');
  else writer.parts.push(char);
  return index + 1;
}

/** Open an object or array; an empty one stays on one line, e.g. `{}`. */
function open(
  text: string,
  index: number,
  closer: string,
  writer: Writer,
): number {
  let next = index + 1;
  while (blank.has(text.charAt(next))) next += 1;
  if (text.charAt(next) === closer) {
    writer.parts.push(text.charAt(index), closer);
    return next + 1;
  }
  writer.depth += 1;
  writer.parts.push(text.charAt(index), newline(writer));
  return index + 1;
}

function newline(writer: Writer): string {
  return writer.pretty ? `\n${'  '.repeat(writer.depth)}` : '';
}

/** Index after the closing quote of the string starting at `start`. */
function stringEnd(text: string, start: number): number {
  let index = start + 1;
  while (index < text.length) {
    const char = text.charAt(index);
    if (char === '\\') index += 2;
    else if (char === '"') return index + 1;
    else index += 1;
  }
  return text.length;
}
