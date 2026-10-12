// SPDX-License-Identifier: Apache-2.0
/** Remove only the top-level _id without coercing EJSON or large JSON numbers. */
export function removeDocumentId(text: string): string | null {
  try {
    const value: unknown = JSON.parse(text);
    if (value === null || typeof value !== 'object' || Array.isArray(value))
      return null;
  } catch {
    return null;
  }
  const source = text.trim();
  const members = entries(source);
  const remaining = members.filter((member) => {
    const key = /^\s*("(?:[^"\\]|\\.)*")\s*:/.exec(member)?.[1];
    return !key || (JSON.parse(key) as unknown) !== '_id';
  });
  return `{${remaining.join(',')}}`;
}
function entries(source: string): string[] {
  const members: string[] = [];
  let depth = 0,
    quoted = false,
    escaped = false,
    start = 1;
  for (let index = 1; index < source.length - 1; index++) {
    const char = source.charAt(index);
    if (quoted) {
      if (escaped) escaped = false;
      else if (char === '\\') escaped = true;
      else if (char === '"') quoted = false;
    } else if (char === '"') quoted = true;
    else if (char === '{' || char === '[') depth++;
    else if (char === '}' || char === ']') depth--;
    else if (char === ',' && depth === 0) {
      members.push(source.slice(start, index));
      start = index + 1;
    }
  }
  const last = source.slice(start, -1);
  if (last.trim()) members.push(last);
  return members;
}
