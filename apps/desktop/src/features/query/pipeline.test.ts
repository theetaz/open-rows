// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { pipelineQuery } from './pipeline';
it('previews only the requested prefix with a sample limit', () => {
  const query = pipelineQuery('a"b', '[{"$match":{}},{"$sort":{"n":1}}]', 0);
  expect(query).toContain('"$limit":100');
  expect(query).not.toContain('$sort');
  expect(query).toContain('"a\\"b"');
});
it('refuses writes and server JavaScript in previews, including nested stages', () => {
  for (const pipeline of [
    '[{"$out":"x"}]',
    '[{"$facet":{"x":[{"$merge":"x"}]}}]',
    '[{"$project":{"x":{"$function":{}}}}]',
  ])
    expect(() => pipelineQuery('x', pipeline, 0)).toThrow();
  expect(() => pipelineQuery('x', '{}')).toThrow();
});

it('preserves exact integer digits in a preview', () => {
  expect(
    pipelineQuery('x', '[{"$match":{"n":9007199254740993}}]', 0),
  ).toContain('9007199254740993');
});
