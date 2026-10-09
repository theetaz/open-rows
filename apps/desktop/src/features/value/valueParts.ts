// SPDX-License-Identifier: Apache-2.0
/** The parts of a PostgreSQL value the inspector lists beside its text. */
export type ValueParts =
  | { kind: 'array'; items: (string | null)[] }
  | {
      kind: 'range';
      empty: boolean;
      lower: string | null;
      upper: string | null;
      lowerInclusive: boolean;
      upperInclusive: boolean;
    }
  | { kind: 'network'; address: string; prefix: string | null };

/** Unquote one array or range item (`"a \"b\""` → `a "b"`). */
function unquote(item: string): string {
  return item.startsWith('"')
    ? item.slice(1, -1).replace(/\\(.)/g, '$1')
    : item.trim();
}

/** Split `body` on top-level `separator`s, keeping quoted items whole. */
function splitItems(body: string, separator: string): string[] {
  const items: string[] = [];
  let current = '';
  let quoted = false;
  for (let index = 0; index < body.length; index++) {
    const char = body.charAt(index);
    if (char === '\\' && quoted) {
      current += char + body.charAt(index + 1);
      index++;
      continue;
    }
    if (char === '"') quoted = !quoted;
    if (char === separator && !quoted) {
      items.push(current);
      current = '';
    } else current += char;
  }
  items.push(current);
  return items;
}

/** Items of a one-dimensional array literal such as `{1,NULL,"a b"}`. */
export function arrayItems(text: string): (string | null)[] | null {
  if (!text.startsWith('{') || !text.endsWith('}')) return null;
  const body = text.slice(1, -1);
  if (body === '') return [];
  if (/^\s*\{/.test(body)) return null;
  return splitItems(body, ',').map((item) =>
    item.trim().toUpperCase() === 'NULL' ? null : unquote(item),
  );
}

function rangeParts(text: string): ValueParts | null {
  if (text === 'empty')
    return {
      kind: 'range',
      empty: true,
      lower: null,
      upper: null,
      lowerInclusive: false,
      upperInclusive: false,
    };
  const match = /^([[(])(.*)([\])])$/.exec(text);
  const bounds = match?.[2] === undefined ? [] : splitItems(match[2], ',');
  const [lower, upper] = bounds;
  if (
    !match ||
    bounds.length !== 2 ||
    lower === undefined ||
    upper === undefined
  )
    return null;
  return {
    kind: 'range',
    empty: false,
    lower: lower === '' ? null : unquote(lower),
    upper: upper === '' ? null : unquote(upper),
    lowerInclusive: match[1] === '[',
    upperInclusive: match[3] === ']',
  };
}

/**
 * Arrays (`integer[]`), ranges (`int4range`) and network addresses
 * (`inet`, `cidr`) split into their parts; `null` for anything else.
 */
export function valueParts(
  type: string | null,
  text: string | null,
): ValueParts | null {
  if (!type || text === null) return null;
  if (type.endsWith('[]')) {
    const items = arrayItems(text);
    return items ? { kind: 'array', items } : null;
  }
  if (type.endsWith('range') && !type.endsWith('multirange'))
    return rangeParts(text);
  if (type === 'inet' || type === 'cidr') {
    const [address = text, prefix] = text.split('/');
    return { kind: 'network', address, prefix: prefix ?? null };
  }
  return null;
}
