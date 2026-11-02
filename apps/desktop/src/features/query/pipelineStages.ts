// SPDX-License-Identifier: Apache-2.0
/** Source slices of a validated JSON array; preserve large numbers and escapes. */
export function pipelineStages(text: string): string[] {
  const source = text.trim();
  const stages: string[] = [];
  let depth = 0;
  let quoted = false;
  let escaped = false;
  let start = 1;
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
      stages.push(source.slice(start, index));
      start = index + 1;
    }
  }
  const last = source.slice(start, -1);
  if (last.trim()) stages.push(last);
  return stages;
}
