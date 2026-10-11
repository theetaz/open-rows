// SPDX-License-Identifier: Apache-2.0
import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import {
  cancelAssistant,
  explainAssistant,
  getAiSharing,
  previewAssistant,
  runAssistant,
  verifyAiProvider,
} from '@openrows/ipc';
import type { AgentEvent, SharingPolicy } from '@openrows/ipc';
import { AssistantPanel } from './AssistantPanel';
import { clearConversation, conversationOf } from './conversation';
import type { AssistantProps } from './context';
import { aiSettings, parseAiSettings } from '../settings/ai/aiSettings';
import { profile } from '../../test/ipcStub';
import { samplePlan } from '../../test/planStub';
const security = vi.hoisted(() => ({
  locked: false,
  available: true,
  notice: null,
}));
vi.mock('../security/useSecurity', () => ({ useSecurity: () => security }));
vi.mock('@openrows/ipc', async () => ({
  ...(await import('../../test/ipcStub')).ipcStub(),
  getAiSharing: vi.fn(),
  runAssistant: vi.fn(),
  cancelAssistant: vi.fn().mockResolvedValue(undefined),
  previewAssistant: vi.fn(),
  explainAssistant: vi.fn(),
  cancelAiVerification: vi.fn().mockResolvedValue(undefined),
  verifyAiProvider: vi.fn(),
}));
const policy: Required<SharingPolicy> = {
  level: 'sampleRows',
  providers: ['fixture'],
  localOnly: false,
  telemetry: true,
  productionConfirmed: false,
  rules: [],
};
const preview = {
  payload: '{"query":"SELECT redacted"}',
  bytes: 27,
  tokens: 27,
  categories: ['query'] as const,
  truncated: false,
};
let events: ((event: AgentEvent) => void) | null = null;
beforeEach(() => {
  clearConversation('tab');
  vi.clearAllMocks();
  security.locked = false;
  aiSettings.set(
    parseAiSettings({
      enabled: true,
      providers: [
        {
          id: 'fixture',
          name: 'Fixture',
          kind: 'openAi',
          baseUrl: 'https://api.openai.com/v1/',
          allowInsecureRemote: false,
          secretRef: '',
        },
      ],
      defaultProvider: 'fixture',
      defaultModel: 'model',
    }),
  );
  vi.mocked(getAiSharing).mockResolvedValue(policy);
  vi.mocked(previewAssistant).mockResolvedValue({
    ...preview,
    categories: [...preview.categories],
  });
  vi.mocked(explainAssistant).mockResolvedValue({
    ...preview,
    categories: [...preview.categories],
  });
  vi.mocked(runAssistant).mockImplementation((_request, sink) => {
    events = sink;
    return new Promise(() => undefined);
  });
});
function props(overrides: Partial<AssistantProps> = {}): AssistantProps {
  return {
    tabId: 'tab',
    profile: profile('local'),
    database: 'orders',
    connected: true,
    sql: () => 'SELECT id FROM public.orders',
    lastError: null,
    plan: null,
    onInsert: vi.fn(),
    onOpen: vi.fn(),
    ...overrides,
  };
}
async function ready(input = props()) {
  const user = userEvent.setup();
  const view = render(<AssistantPanel {...input} />);
  await waitFor(() => {
    expect(
      screen.queryByText('Loading sharing policy…'),
    ).not.toBeInTheDocument();
  });
  return { user, ...view };
}
it('renders streaming text, collapsible trace, safe proposals and insert/open/explain actions', async () => {
  const input = props();
  const { user } = await ready(input);
  expect(runAssistant).not.toHaveBeenCalled();
  expect(verifyAiProvider).not.toHaveBeenCalled();
  await user.type(
    screen.getByLabelText('Assistant question'),
    'Show recent orders',
  );
  await user.click(screen.getByRole('button', { name: 'Send' }));
  act(() => {
    events?.({ type: 'text', delta: 'Here is ' });
    events?.({ type: 'text', delta: 'a proposal' });
    events?.({
      type: 'trace',
      trace: {
        step: 2,
        tool: 'run_read_query',
        arguments: 'sql: 20 bytes',
        bytes: 80,
        redacted: true,
        durationMs: 2,
        status: 'succeeded',
      },
    });
    events?.({ type: 'proposal', sql: 'SELECT $1, ${literal}' });
    events?.({ type: 'done', tokens: 100 });
  });
  expect(screen.getByText('Here is a proposal')).toBeVisible();
  await user.click(screen.getByText(/run_read_query · succeeded/));
  expect(screen.getByText(/80 bytes · redacted/)).toBeVisible();
  await user.click(screen.getByRole('button', { name: 'Insert into editor' }));
  await user.click(screen.getByRole('button', { name: 'Open in new tab' }));
  expect(input.onInsert).toHaveBeenCalledWith('SELECT $1, ${literal}');
  expect(input.onOpen).toHaveBeenCalledWith('SELECT $1, ${literal}');
  await user.click(screen.getByRole('button', { name: /^Explain$/ }));
  expect(explainAssistant).toHaveBeenCalledWith(
    expect.objectContaining({ query: 'SELECT $1, ${literal}' }),
  );
  expect(JSON.stringify(vi.mocked(explainAssistant).mock.calls)).not.toContain(
    'analyze',
  );
});
it('previews the exact selected context without provider calls and cancels a running turn', async () => {
  const { user } = await ready();
  await user.click(screen.getByRole('button', { name: 'Context preview' }));
  expect(
    await screen.findByLabelText('Exact assistant context'),
  ).toHaveTextContent(preview.payload);
  expect(runAssistant).not.toHaveBeenCalled();
  await user.type(
    screen.getByLabelText('Assistant question'),
    'Explain orders',
  );
  await user.click(screen.getByRole('button', { name: 'Send' }));
  const clock = vi.spyOn(performance, 'now').mockReturnValue(100);
  try {
    act(() => {
      events?.({
        type: 'trace',
        trace: {
          step: 2,
          tool: 'run_read_query',
          arguments: 'sql: 8 bytes',
          bytes: 0,
          redacted: false,
          durationMs: 0,
          status: 'running',
        },
      });
    });
    clock.mockReturnValue(125);
    await user.click(screen.getByRole('button', { name: 'Cancel assistant' }));
    expect(cancelAssistant).toHaveBeenCalledWith(expect.any(String));
    expect(conversationOf('tab').running).toBeNull();
    expect(screen.getByText('Cancelled')).toBeVisible();
    expect(
      screen.getByText('run_read_query · Cancelled · 25 ms'),
    ).toBeVisible();
  } finally {
    clock.mockRestore();
  }
});
it('quick actions share query, fixed last-error code and normalized plan-only evidence', async () => {
  const input = props({
    lastError: {
      statement: 0,
      level: 'error',
      text: 'private raw vendor error',
      code: '42601',
    },
    plan: samplePlan({ analyzed: false }),
  });
  const { user } = await ready(input);
  await user.click(screen.getByRole('button', { name: 'Fix this error' }));
  expect(vi.mocked(runAssistant).mock.calls.at(-1)?.[0].query).toBe(
    input.sql(),
  );
  expect(
    vi.mocked(runAssistant).mock.calls.at(-1)?.[0].question.errorCode,
  ).toBe('42601');
  expect(JSON.stringify(vi.mocked(runAssistant).mock.calls)).not.toContain(
    'private raw vendor error',
  );
  act(() => {
    events?.({ type: 'done', tokens: 1 });
  });
  await user.click(screen.getByRole('button', { name: 'Optimise from plan' }));
  expect(
    vi.mocked(runAssistant).mock.calls.at(-1)?.[0].plan.length,
  ).toBeGreaterThan(0);
  act(() => {
    events?.({ type: 'done', tokens: 1 });
  });
  await user.click(screen.getByRole('button', { name: 'Explain this query' }));
  expect(vi.mocked(runAssistant).mock.calls.at(-1)?.[0].question.question).toBe(
    'Explain this query',
  );
});
it.each(['off', 'provider', 'none', 'lock', 'production'] as const)(
  'shows the %s disabled state and sends no requests',
  async (state) => {
    if (state === 'off')
      aiSettings.set({ ...aiSettings.get(), enabled: false });
    if (state === 'provider')
      aiSettings.set({
        ...aiSettings.get(),
        providers: [],
        defaultProvider: '',
      });
    if (state === 'none')
      vi.mocked(getAiSharing).mockResolvedValue({ ...policy, level: 'none' });
    if (state === 'lock') security.locked = true;
    const input = props(
      state === 'production'
        ? {
            profile: profile('prod', {
              env: { kind: 'prod', label: null, color: '#dc2626' },
            }),
          }
        : {},
    );
    await ready(input);
    expect(screen.getByRole('button', { name: 'Send' })).toBeDisabled();
    expect(
      screen.getByRole('button', { name: 'Context preview' }),
    ).toBeDisabled();
    expect(runAssistant).not.toHaveBeenCalled();
    expect(previewAssistant).not.toHaveBeenCalled();
    expect(verifyAiProvider).not.toHaveBeenCalled();
  },
);
