// SPDX-License-Identifier: Apache-2.0
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { saveDiagram, schemaDiagram } from '@openrows/ipc';
import { ErDiagram } from './index';
import { svgToPng } from './rasterize';
import { shop } from './testDiagram';

vi.mock('@openrows/ipc', () => ({
  schemaDiagram: vi.fn(),
  saveDiagram: vi.fn(),
}));
vi.mock('./rasterize', async (original) => ({
  ...(await original<typeof import('./rasterize')>()),
  svgToPng: vi.fn(() => Promise.resolve(new Uint8Array([137, 80, 78, 71]))),
}));

beforeEach(() => {
  vi.mocked(schemaDiagram).mockResolvedValue(shop);
  vi.mocked(saveDiagram).mockResolvedValue('/tmp/public.svg');
});

function renderDiagram(focus?: string) {
  const onOpen = vi.fn();
  const onClose = vi.fn();
  render(
    <ThemeProvider>
      <ErDiagram
        profileId="p1"
        database="shop"
        schema="public"
        focus={focus}
        onOpen={onOpen}
        onClose={onClose}
      />
    </ThemeProvider>,
  );
  return { onOpen, onClose };
}

const tables = async (): Promise<HTMLElement[]> =>
  screen.findAllByRole('button', { name: /^Open table / });

it('draws every table with its columns and key marks', async () => {
  renderDiagram();
  expect(await tables()).toHaveLength(5);
  expect(schemaDiagram).toHaveBeenCalledWith({
    profileId: 'p1',
    database: 'shop',
    schema: 'public',
  });
  const orders = screen.getByRole('button', { name: 'Open table orders' });
  expect(orders).toHaveTextContent('PKidbigint');
  expect(orders).toHaveTextContent('FKcustomer_idinteger');
  expect(document.querySelectorAll('.erd-edge')).toHaveLength(2);
  expect(screen.getByText('5 tables · 2 relationships')).toBeInTheDocument();
});

it('highlights matches of the search and opens a clicked table', async () => {
  const user = userEvent.setup();
  const { onOpen, onClose } = renderDiagram();
  await tables();
  await user.type(screen.getByLabelText('Find table'), 'line');
  expect(screen.getByText('1 match')).toBeInTheDocument();
  const lines = screen.getByRole('button', { name: 'Open table lines' });
  expect(lines).toHaveAttribute('data-match');
  expect(
    screen.getByRole('button', { name: 'Open table orders' }),
  ).not.toHaveAttribute('data-match');
  await user.click(lines);
  expect(onClose).toHaveBeenCalled();
  expect(onOpen).toHaveBeenCalledWith('lines');
});

it('zooms and fits from the toolbar', async () => {
  const user = userEvent.setup();
  renderDiagram('orders');
  await tables();
  expect(screen.getByText('100%')).toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: 'Zoom in' }));
  expect(screen.getByText('125%')).toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: 'Fit to view' }));
  expect(screen.queryByText('125%')).not.toBeInTheDocument();
});

it('exports SVG text and PNG bytes through the native save panel', async () => {
  const user = userEvent.setup();
  renderDiagram();
  await tables();
  await user.click(screen.getByRole('button', { name: 'Export SVG…' }));
  await waitFor(() => {
    expect(saveDiagram).toHaveBeenCalledTimes(1);
  });
  const [name, file] = vi.mocked(saveDiagram).mock.calls[0] ?? [];
  expect(name).toBe('public');
  expect(file?.format).toBe('svg');
  const svg = file?.format === 'svg' ? file.text : '';
  expect(svg).toMatch(/^<svg xmlns="http:\/\/www.w3.org\/2000\/svg"/);
  expect(svg).toContain('customers');
  expect(svg).toContain('erd-edge');
  expect(await screen.findByText('Saved /tmp/public.svg')).toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: 'Export PNG…' }));
  await waitFor(() => {
    expect(saveDiagram).toHaveBeenCalledWith('public', {
      format: 'png',
      bytes: [137, 80, 78, 71],
    });
  });
  expect(vi.mocked(svgToPng).mock.calls[0]?.[0]).toContain('customers');
});

it('reports read failures', async () => {
  vi.mocked(schemaDiagram).mockRejectedValueOnce(
    new Error('permission denied'),
  );
  renderDiagram();
  expect(await screen.findByRole('alert')).toHaveTextContent(
    'permission denied',
  );
});

it('says when a schema has no tables', async () => {
  vi.mocked(schemaDiagram).mockResolvedValueOnce({
    ...shop,
    tables: [],
    foreignKeys: [],
  });
  renderDiagram();
  expect(await screen.findByText('public has no tables')).toBeInTheDocument();
});

it('notes a truncated diagram', async () => {
  vi.mocked(schemaDiagram).mockResolvedValueOnce({
    ...shop,
    truncated: true,
    limit: 5,
  });
  renderDiagram();
  expect(
    await screen.findByText(/showing the first 5 tables by name/),
  ).toBeInTheDocument();
});
