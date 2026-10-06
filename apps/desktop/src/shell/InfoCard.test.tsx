// SPDX-License-Identifier: Apache-2.0
import { render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { getAppInfo } from '@openrows/ipc';
import { InfoCard } from './InfoCard';

vi.mock('@openrows/ipc', () => ({ getAppInfo: vi.fn() }));

beforeEach(() => {
  vi.mocked(getAppInfo).mockReset();
});

describe('InfoCard', () => {
  it('shows loading while IPC is pending', () => {
    vi.mocked(getAppInfo).mockReturnValue(new Promise(() => {}));
    render(<InfoCard />);
    expect(screen.getByRole('status')).toHaveTextContent(
      'Loading application information',
    );
  });

  it('shows the name, version and debug profile returned through IPC', async () => {
    vi.mocked(getAppInfo).mockResolvedValue({
      name: 'OpenRows',
      version: '0.1.0',
      buildProfile: 'debug',
    });
    render(<InfoCard />);
    expect(
      await screen.findByRole('heading', { name: 'OpenRows' }),
    ).toBeVisible();
    expect(screen.getByText('Version 0.1.0')).toBeVisible();
    expect(screen.getByText('debug')).toBeVisible();
    expect(getAppInfo).toHaveBeenCalledOnce();
    expect(screen.queryByRole('status')).not.toBeInTheDocument();
  });

  it('shows an IPC failure', async () => {
    vi.mocked(getAppInfo).mockRejectedValue(new Error('Command unavailable'));
    render(<InfoCard />);
    expect(await screen.findByRole('alert')).toHaveTextContent(
      'Command unavailable',
    );
  });

  it('shows string rejections from Tauri', async () => {
    vi.mocked(getAppInfo).mockRejectedValue('Permission denied');
    render(<InfoCard />);
    expect(await screen.findByRole('alert')).toHaveTextContent(
      'Permission denied',
    );
  });
});
