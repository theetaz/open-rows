// SPDX-License-Identifier: Apache-2.0
import { render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { getAppInfo } from '@openrows/ipc';
import { AppFooter } from './AppFooter';

vi.mock('@openrows/ipc', () => ({ getAppInfo: vi.fn() }));

beforeEach(() => {
  vi.mocked(getAppInfo).mockReset();
});

describe('AppFooter', () => {
  it('shows loading while IPC is pending', () => {
    vi.mocked(getAppInfo).mockReturnValue(new Promise(() => {}));
    render(<AppFooter />);
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
    render(<AppFooter />);
    expect(await screen.findByText('OpenRows 0.1.0 · debug')).toBeVisible();
    expect(getAppInfo).toHaveBeenCalledOnce();
    expect(screen.queryByRole('status')).not.toBeInTheDocument();
  });

  it('shows an IPC failure', async () => {
    vi.mocked(getAppInfo).mockRejectedValue(new Error('Command unavailable'));
    render(<AppFooter />);
    expect(await screen.findByRole('alert')).toHaveTextContent(
      'Command unavailable',
    );
  });

  it('shows string rejections from Tauri', async () => {
    vi.mocked(getAppInfo).mockRejectedValue('Permission denied');
    render(<AppFooter />);
    expect(await screen.findByRole('alert')).toHaveTextContent(
      'Permission denied',
    );
  });
});
