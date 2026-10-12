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
  fireEvent.click(screen.getByRole('button', { name: /^tree$/ }));
  expect(screen.getByRole('list', { name: 'JSON tree' })).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: /^json$/ }));
  expect(screen.getByText('{"nested":{"n":1}}')).toBeInTheDocument();
  expect(screen.getByRole('button', { name: 'Next document' })).toBeDisabled();
});
