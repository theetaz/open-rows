// SPDX-License-Identifier: Apache-2.0
import { fireEvent, render, screen } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { PipelineEditor } from './PipelineEditor';
it('runs a sampled prefix and refuses a write-producing preview', () => {
  const run = vi.fn();
  render(<PipelineEditor run={run} disabled={false} />);
  fireEvent.click(screen.getByRole('button', { name: 'Pipeline editor' }));
  fireEvent.change(screen.getByRole('textbox', { name: 'Collection' }), {
    target: { value: 'orders' },
  });
  fireEvent.change(screen.getByRole('textbox', { name: 'Pipeline JSON' }), {
    target: { value: '[{"$match":{}},{"$out":"output"}]' },
  });
  fireEvent.click(screen.getByRole('button', { name: 'Preview stage 1' }));
  expect(run).toHaveBeenCalledWith(
    'db.getCollection("orders").aggregate([{"$match":{}},{"$limit":100}])',
  );
  run.mockClear();
  fireEvent.click(screen.getByRole('button', { name: 'Preview stage 2' }));
  expect(run).not.toHaveBeenCalled();
  expect(screen.getByRole('alert')).toHaveTextContent('cannot contain $out');
});
