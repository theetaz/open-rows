// SPDX-License-Identifier: Apache-2.0
import { beforeEach, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {
  getAiSharing,
  saveAiSharing,
  previewAiContext,
  listAiAudit,
  purgeAiAudit,
} from '@openrows/ipc';
import type { SharingPolicy } from '@openrows/ipc';
import ConnectionSharing from './ConnectionSharing';
import { ContextPreview } from './ContextPreview';
import { AuditViewer } from './AuditViewer';
import { aiSettings, parseAiSettings } from './aiSettings';
import { reloadSettings } from '../localSetting';
vi.mock('@openrows/ipc', async () => ({
  ...(await import('../../../test/ipcStub')).ipcStub(),
  getAiSharing: vi.fn(),
  saveAiSharing: vi.fn(),
  previewAiContext: vi.fn(),
  listAiAudit: vi.fn(),
  purgeAiAudit: vi.fn(),
}));
const provider = {
  id: 'fixture',
  name: 'Fixture',
  kind: 'openAi' as const,
  baseUrl: 'https://api.openai.com/v1/',
  allowInsecureRemote: false,
  secretRef: '',
};
const defaultPolicy: Required<SharingPolicy> = {
  level: 'none',
  providers: [],
  localOnly: false,
  telemetry: false,
  productionConfirmed: false,
  rules: [],
};
beforeEach(() => {
  vi.clearAllMocks();
  aiSettings.set(parseAiSettings({ enabled: true, providers: [provider] }));
  reloadSettings();
  vi.mocked(getAiSharing).mockResolvedValue(defaultPolicy);
  vi.mocked(saveAiSharing).mockResolvedValue(undefined);
  vi.mocked(previewAiContext).mockResolvedValue({
    payload: '{"query":"SELECT ‹redacted›"}',
    bytes: 33,
    tokens: 33,
    categories: ['query'],
    truncated: true,
  });
  vi.mocked(listAiAudit).mockResolvedValue([]);
  vi.mocked(purgeAiAudit).mockResolvedValue(undefined);
});
it('edits level, provider allowlist, local-only, telemetry and rules before native save', async () => {
  const user = userEvent.setup();
  render(<ConnectionSharing id="connection" production={false} />);
  await screen.findByLabelText('Sharing level');
  expect(screen.getByLabelText('Sharing level')).toHaveValue('none');
  await user.selectOptions(
    screen.getByLabelText('Sharing level'),
    'sampleRows',
  );
  await user.click(screen.getByLabelText('Allow Fixture'));
  await user.click(screen.getByLabelText('Local models only'));
  await user.click(
    screen.getByLabelText('Share aggregate statistics and plans'),
  );
  await user.type(
    screen.getByLabelText('Schema.table.column pattern'),
    'public.people.note',
  );
  await user.click(screen.getByRole('button', { name: 'Add redaction rule' }));
  expect(
    screen.getByRole('button', { name: 'Preview context' }),
  ).toBeDisabled();
  await user.click(screen.getByRole('button', { name: 'Save sharing policy' }));
  await screen.findByText('Sharing policy saved.');
  expect(saveAiSharing).toHaveBeenCalledWith('connection', {
    ...defaultPolicy,
    level: 'sampleRows',
    providers: ['fixture'],
    localOnly: true,
    telemetry: true,
    rules: [{ kind: 'glob', pattern: 'public.people.note' }],
  });
  expect(previewAiContext).not.toHaveBeenCalled();
});
it('requires fresh production confirmation and allows turning sharing off without it', async () => {
  vi.mocked(getAiSharing).mockResolvedValue({
    ...defaultPolicy,
    productionConfirmed: true,
  });
  const user = userEvent.setup();
  render(<ConnectionSharing id="production" production />);
  await screen.findByLabelText('Sharing level');
  const confirmation = screen.getByLabelText(
    'I confirm sharing production context with the allowed providers',
  );
  expect(confirmation).not.toBeChecked();
  await user.selectOptions(
    screen.getByLabelText('Sharing level'),
    'schemaOnly',
  );
  await user.click(screen.getByLabelText('Allow Fixture'));
  const save = screen.getByRole('button', { name: 'Save sharing policy' });
  expect(save).toBeDisabled();
  await user.click(confirmation);
  await user.click(save);
  await waitFor(() => {
    expect(saveAiSharing).toHaveBeenCalledWith(
      'production',
      expect.objectContaining({
        productionConfirmed: true,
        level: 'schemaOnly',
      }),
    );
  });
  await user.selectOptions(screen.getByLabelText('Sharing level'), 'none');
  await user.click(confirmation);
  expect(save).toBeEnabled();
});
it('renders exact offline preview and clears it when selected context changes', async () => {
  const user = userEvent.setup();
  render(
    <ContextPreview id="connection" providers={[provider]} disabled={false} />,
  );
  await user.type(
    screen.getByLabelText('Schema definitions'),
    'CREATE TABLE t(id int)',
  );
  await user.type(screen.getByLabelText('Query SQL'), "SELECT 'private'");
  await user.click(screen.getByRole('button', { name: 'Preview context' }));
  await screen.findByLabelText('Exact context payload');
  expect(screen.getByLabelText('Exact context payload').textContent).toBe(
    '{"query":"SELECT ‹redacted›"}',
  );
  expect(screen.getByText(/33 bytes/)).toHaveTextContent(
    'Truncated to sharing limits',
  );
  expect(previewAiContext).toHaveBeenCalledWith('connection', 'fixture', {
    schema: [{ name: 'selected', ddl: 'CREATE TABLE t(id int)' }],
    query: "SELECT 'private'",
    plan: [],
    stats: [],
    samples: [],
  });
  await user.type(screen.getByLabelText('Query SQL'), ' ');
  expect(
    screen.queryByLabelText('Exact context payload'),
  ).not.toBeInTheDocument();
  expect(listAiAudit).not.toHaveBeenCalled();
});
it('passes selected rows/plans through the reusable preview and displays a refusal', async () => {
  vi.mocked(previewAiContext).mockRejectedValue(
    new Error('Data-sharing level is too low'),
  );
  const bundle = {
    samples: [{ table: 't', columns: ['email'], rows: [['private']] }],
    plan: [],
    stats: [],
    query: null,
    schema: [],
  };
  const user = userEvent.setup();
  render(
    <ContextPreview
      id="connection"
      providers={[provider]}
      disabled={false}
      bundle={bundle}
    />,
  );
  await user.click(screen.getByRole('button', { name: 'Preview context' }));
  expect(await screen.findByRole('alert')).toHaveTextContent(
    'Data-sharing level is too low',
  );
  expect(previewAiContext).toHaveBeenCalledWith(
    'connection',
    'fixture',
    bundle,
  );
});
it('shows metadata only and purges after an explicit local review', async () => {
  vi.mocked(listAiAudit).mockResolvedValue([
    {
      time: '0',
      provider: 'fixture',
      connection: 'connection',
      level: 'schemaOnly',
      bytes: 64,
      hash: 'hash',
      categories: ['schema'],
    },
  ]);
  const user = userEvent.setup();
  render(<AuditViewer />);
  await screen.findByText('fixture');
  await user.click(screen.getByRole('button', { name: 'Purge audit…' }));
  expect(purgeAiAudit).not.toHaveBeenCalled();
  await user.click(
    screen.getByRole('button', { name: 'Delete audit records' }),
  );
  await screen.findByText('No outbound database context requests recorded.');
  expect(purgeAiAudit).toHaveBeenCalledTimes(1);
});
it('keeps unsaved policy after persistence failure and disables preview while AI is off', async () => {
  aiSettings.set(parseAiSettings({ providers: [provider] }));
  vi.mocked(saveAiSharing).mockRejectedValue(
    new Error('AI sharing store is unavailable'),
  );
  const user = userEvent.setup();
  render(<ConnectionSharing id="connection" production={false} />);
  await screen.findByLabelText('Sharing level');
  await user.selectOptions(
    screen.getByLabelText('Sharing level'),
    'schemaOnly',
  );
  await user.click(screen.getByRole('button', { name: 'Save sharing policy' }));
  expect(await screen.findByRole('alert')).toHaveTextContent(
    'AI sharing store is unavailable',
  );
  expect(screen.getByLabelText('Sharing level')).toHaveValue('schemaOnly');
  expect(
    screen.getByRole('button', { name: 'Preview context' }),
  ).toBeDisabled();
});

it('opens data sharing from Settings AI and from a saved connection on demand', async () => {
  const { SharingDisclosure } = await import('./SharingDisclosure');
  const { AiSharingDisclosure } =
    await import('../../connections/AiSharingDisclosure');
  const user = userEvent.setup();
  const global = render(<SharingDisclosure />);
  expect(listAiAudit).not.toHaveBeenCalled();
  await user.click(screen.getByRole('button', { name: 'Data sharing' }));
  await screen.findByLabelText('Sharing connection');
  expect(listAiAudit).toHaveBeenCalledTimes(1);
  global.unmount();
  render(<AiSharingDisclosure id="connection" production={false} />);
  expect(getAiSharing).not.toHaveBeenCalled();
  await user.click(screen.getByRole('button', { name: 'AI data sharing' }));
  await screen.findByLabelText('Sharing level');
  expect(getAiSharing).toHaveBeenCalledWith('connection');
});
