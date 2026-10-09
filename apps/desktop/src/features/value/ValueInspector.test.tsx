// SPDX-License-Identifier: Apache-2.0
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { resultCellInfo, resultCellValue } from '@openrows/ipc';
import { ValueInspector } from './ValueInspector';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

const address = { resultId: 'r1', row: 0, column: 2 };

const encoded = (text: string): ArrayBuffer =>
  new TextEncoder().encode(text).buffer;

function mount(
  type: string | null,
  text: string | null,
): ReturnType<typeof userEvent.setup> {
  render(
    <ThemeProvider>
      <ValueInspector
        address={address}
        name="payload"
        type={type}
        text={text}
      />
    </ThemeProvider>,
  );
  return userEvent.setup();
}

beforeEach(() => {
  vi.mocked(resultCellInfo).mockReset();
  vi.mocked(resultCellValue).mockReset();
});

it('shows a JSON document as a tree and switches to its exact text', async () => {
  vi.mocked(resultCellInfo).mockResolvedValue({ kind: 'text', byteLen: 27 });
  vi.mocked(resultCellValue).mockResolvedValue(
    encoded('{"name":"ada","tags":[1,2]}'),
  );
  const user = mount('jsonb', '{"name":"ada"}');
  expect(await screen.findByRole('list', { name: 'JSON tree' })).toBeVisible();
  expect(screen.getByText('"ada"')).toBeVisible();
  expect(resultCellInfo).toHaveBeenCalledWith(address);
  await user.click(screen.getByRole('radio', { name: 'Text' }));
  expect(screen.getByText('{"name":"ada","tags":[1,2]}')).toBeVisible();
});

it('shows NULL without fetching a value', async () => {
  vi.mocked(resultCellInfo).mockResolvedValue({ kind: 'null', byteLen: 0 });
  mount(null, null);
  expect(await screen.findByText('NULL')).toBeVisible();
  expect(resultCellValue).not.toHaveBeenCalled();
});

it('shows the display text of values without bytes', async () => {
  vi.mocked(resultCellInfo).mockResolvedValue({ kind: 'other', byteLen: 0 });
  mount('int8', '42');
  expect(await screen.findByText('42')).toBeVisible();
  expect(resultCellValue).not.toHaveBeenCalled();
});

it('waits for "Load full value" above the automatic size, then shows hex', async () => {
  const size = 3 * 1024 * 1024;
  vi.mocked(resultCellInfo).mockResolvedValue({
    kind: 'binary',
    byteLen: size,
  });
  vi.mocked(resultCellValue).mockResolvedValue(new Uint8Array(size).buffer);
  const user = mount('bytea', null);
  expect(
    await screen.findByText(/larger than the automatic preview/),
  ).toBeVisible();
  expect(resultCellValue).not.toHaveBeenCalled();
  await user.click(screen.getByRole('button', { name: 'Load full value' }));
  expect(await screen.findByLabelText('Hex dump')).toHaveTextContent(
    '00000000 00 00',
  );
  expect(screen.getByText(/Showing the first 64 KB of 3.0 MB/)).toBeVisible();
  expect(resultCellValue).toHaveBeenCalledWith(address);
});

it('previews a PNG by its magic bytes and offers the hex view', async () => {
  const png = Uint8Array.from([
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 1, 2,
  ]);
  vi.mocked(resultCellInfo).mockResolvedValue({
    kind: 'binary',
    byteLen: png.length,
  });
  vi.mocked(resultCellValue).mockResolvedValue(png.buffer);
  const user = mount('bytea', null);
  expect(
    await screen.findByRole('img', { name: 'Image value (image/png)' }),
  ).toBeVisible();
  await user.click(screen.getByRole('radio', { name: 'Hex' }));
  expect(screen.getByLabelText('Hex dump')).toHaveTextContent('89 50 4e 47');
});

it('lists the items of a PostgreSQL array beside its text', async () => {
  vi.mocked(resultCellInfo).mockResolvedValue({ kind: 'other', byteLen: 0 });
  mount('text[]', '{a,NULL,"b c"}');
  const items = await screen.findByRole('list', { name: 'Array items' });
  expect(items).toHaveTextContent('aNULLb c');
  expect(screen.getByText('{a,NULL,"b c"}')).toBeVisible();
});
