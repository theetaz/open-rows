// SPDX-License-Identifier: Apache-2.0
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ArtifactPreview } from './ArtifactPreview';
const { importBoard } = vi.hoisted(() => ({
  importBoard: vi.fn().mockResolvedValue({ reviewed: false }),
}));
vi.mock('@openrows/ipc', () => ({ importBoard, saveReport: vi.fn() }));
describe('artifact preview acceptance', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });
  it('does not save during preview and saves through the paused import boundary only on acceptance', async () => {
    const document =
      '{"version":1,"name":"Preview","params":[],"widgets":[],"refreshSecs":30}';
    render(
      <ArtifactPreview
        connection="fixture"
        kind="board"
        document={document}
        disabled={false}
        evidence={['fixture-sample']}
      />,
    );
    expect(importBoard).not.toHaveBeenCalled();
    expect(
      screen.getByText('Evidence ids: fixture-sample'),
    ).toBeInTheDocument();
    await userEvent.click(
      screen.getByRole('button', { name: 'Accept and save paused board' }),
    );
    expect(importBoard).toHaveBeenCalledWith('fixture', document);
    expect(
      await screen.findByText(
        'Saved paused. Review and enable it in Monitor → Boards.',
      ),
    ).toBeInTheDocument();
  });
  it('renders malicious interpretation as text without any save action', () => {
    const document =
      '{"interpretation":"<script>bad()</script>","uncertainty":"Unknown","evidence":["sample"]}';
    const { container } = render(
      <ArtifactPreview
        connection="fixture"
        kind="finding"
        document={document}
        disabled={false}
      />,
    );
    expect(container.querySelector('script')).toBeNull();
    expect(screen.queryByRole('button')).toBeNull();
    expect(importBoard).not.toHaveBeenCalled();
  });
  it('previews the shared report in a sandbox with escaped model text before export', () => {
    const document = JSON.stringify({
      schema: 'openrows.monitor-report',
      version: 1,
      title: '<script>bad()</script>',
      generatedMs: 1000,
      connection: {
        name: 'fixture',
        engine: 'PostgreSQL',
        database: 'fixture',
      },
      range: { fromMs: 1000, toMs: 1000, label: 'One observation' },
      sections: [],
    });
    render(
      <ArtifactPreview
        connection="fixture"
        kind="report"
        document={document}
        disabled={false}
      />,
    );
    const preview = screen.getByTitle('Report and chart preview');
    expect(preview).toHaveAttribute('sandbox', '');
    expect(preview.getAttribute('srcdoc')).not.toContain('<script>');
    expect(
      screen.getByRole('button', { name: 'Accept and export report…' }),
    ).toBeEnabled();
    expect(importBoard).not.toHaveBeenCalled();
  });
});
