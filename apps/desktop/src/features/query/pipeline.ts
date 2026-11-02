// SPDX-License-Identifier: Apache-2.0
import { pipelineStages } from './pipelineStages';
/** Validate a JSON pipeline and build a bounded read-only stage preview. */
export function pipelineQuery(
  collection: string,
  text: string,
  through?: number,
): string {
  const stages: unknown = JSON.parse(text);
  if (!Array.isArray(stages) || stages.length > 100)
    throw new Error('Use a JSON array of at most 100 stages.');
  if (
    !stages.every(
      (stage: unknown) =>
        stage !== null &&
        typeof stage === 'object' &&
        !Array.isArray(stage) &&
        Object.keys(stage).length === 1,
    )
  )
    throw new Error('Each stage must be an object with one operator.');
  const selected: unknown[] =
    through === undefined ? stages : stages.slice(0, through + 1);
  if (through !== undefined && containsWrite(selected))
    throw new Error(
      'Stage previews cannot contain $out, $merge or server JavaScript.',
    );
  const source = pipelineStages(text);
  const sampled =
    through === undefined
      ? text.trim()
      : `[${[...source.slice(0, through + 1), '{"$limit":100}'].join(',')}]`;
  return `db.getCollection(${JSON.stringify(collection)}).aggregate(${sampled})`;
}
function containsWrite(value: unknown): boolean {
  if (Array.isArray(value)) return value.some(containsWrite);
  if (value === null || typeof value !== 'object') return false;
  return Object.entries(value).some(
    ([key, child]) =>
      ['$out', '$merge', '$function', '$accumulator', '$where'].includes(key) ||
      containsWrite(child),
  );
}
