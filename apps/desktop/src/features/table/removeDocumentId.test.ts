// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { removeDocumentId } from './removeDocumentId';
it('duplicates documents without changing large integers, nested IDs or escapes', () => {
  const text =
    '{"_id":{"$oid":"0123456789abcdef01234567"},"n":9007199254740993,"nested":{"_id":2},"text":"a,b\\"c"}';
  expect(removeDocumentId(text)).toBe(
    '{"n":9007199254740993,"nested":{"_id":2},"text":"a,b\\"c"}',
  );
  expect(removeDocumentId('{"_\\u0069d":1,"n":2}')).toBe('{"n":2}');
  expect(removeDocumentId('[]')).toBeNull();
  expect(removeDocumentId('{broken')).toBeNull();
});
