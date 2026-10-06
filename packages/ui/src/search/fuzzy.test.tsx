// SPDX-License-Identifier: Apache-2.0
import { render } from '@testing-library/react';
import { expect, it } from 'vitest';
import { fuzzyMatch } from './fuzzy';
import { HighlightText } from '../components/HighlightText';
it('matches ordered characters, trims queries and handles missing/empty/case inputs', () => {
  expect(fuzzyMatch('Order_Items', ' OIt ')?.indices).toEqual([0, 6, 7]);
  expect(fuzzyMatch('orders', 'so')).toBeUndefined();
  expect(fuzzyMatch('orders', 'xyz')).toBeUndefined();
  expect(fuzzyMatch('', '')).toEqual({ indices: [], score: 0 });
  expect(fuzzyMatch('orders', '')).toEqual({ indices: [], score: 0 });
  expect(fuzzyMatch('café', 'FÉ')?.indices).toEqual([2, 3]);
});
it('ranks contiguous prefixes above scattered matches without mutating inputs', () => {
  expect(fuzzyMatch('orders', 'ord')?.score).toBeGreaterThan(
    fuzzyMatch('old_records', 'ord')?.score ?? 0,
  );
  expect(fuzzyMatch('order_items', 'it')?.score).toBeGreaterThan(
    fuzzyMatch('orbit', 'it')?.score ?? 0,
  );
});
it('highlights literal matched text without adding text or parsing markup', () => {
  const { container } = render(
    <HighlightText text="<orders>" indices={[1, 2, 3]} />,
  );
  expect(container).toHaveTextContent('<orders>');
  expect(
    Array.from(container.querySelectorAll('mark'))
      .map((mark) => mark.textContent)
      .join(''),
  ).toBe('ord');
  expect(container.querySelector('orders')).toBeNull();
});
