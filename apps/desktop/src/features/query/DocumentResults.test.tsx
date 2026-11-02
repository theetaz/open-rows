// SPDX-License-Identifier: Apache-2.0
import { fireEvent, render, screen } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { DocumentResults } from './DocumentResults';
import type { ResultMeta } from './run';
vi.mock('../value/useCellValue', () => ({
  useCellValue: () => ({
    bytes: new TextEncoder().encode('{"nested":{"n":1}}'),
    problem: null,
    load: vi.fn(),
    loading: false,
  }),
}));
it('switches one document between table, tree and JSON views', () => {
  const result: ResultMeta = {
    id: 'document',
    rows: 1,
    columns: [{ name: '__document', sourceType: 'EJSON' }],
    statement: 0,
    done: true,
    truncated: false,
  };
  render(
    <DocumentResults result={result}>
      <p>Canvas table</p>
    </DocumentResults>,
  );
  expect(screen.getByText('Canvas table')).toBeInTheDocument();
  fireEvent.click(screen.getByRole('radio', { name: 'Tree' }));
  expect(screen.getByRole('list', { name: 'JSON tree' })).toBeInTheDocument();
  fireEvent.click(screen.getByRole('radio', { name: 'JSON' }));
  expect(screen.getByText(/"nested": \{\s+"n": 1\s+\}/)).toBeInTheDocument();
  expect(screen.getByText('Document 1 of 1')).toBeInTheDocument();
  expect(screen.getByRole('button', { name: 'Next document' })).toBeDisabled();
});
