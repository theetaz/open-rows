// SPDX-License-Identifier: Apache-2.0
import { fireEvent, render, screen } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { DiagramCanvas } from './DiagramCanvas';
import { layoutDiagram } from './layout';
import { shop } from './testDiagram';

it('suppresses a drag click while keeping the table selector available', () => {
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(null);
  const onOpen = vi.fn();
  render(
    <DiagramCanvas
      layout={layoutDiagram(shop)}
      view={{ x: 0, y: 0, scale: 1 }}
      matches={new Set()}
      attach={vi.fn()}
      onPointerDown={vi.fn()}
      panned
      onOpen={onOpen}
    />,
  );
  fireEvent.click(screen.getByRole('img', { name: 'ER diagram canvas' }));
  expect(onOpen).not.toHaveBeenCalled();
  fireEvent.change(
    screen.getByRole('combobox', { name: 'Open diagram table' }),
    {
      target: { value: 'orders' },
    },
  );
  expect(onOpen).toHaveBeenCalledWith('orders');
});
