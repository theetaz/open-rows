// SPDX-License-Identifier: Apache-2.0
import { Channel } from '@tauri-apps/api/core';
import { commands } from './generated/bindings';
import type {
  AgentEvent,
  AssistantRequest,
  ContextPreview,
} from './generated/bindings';
import { unwrap } from './connections';
/** Stream one bounded assistant turn; proposals are never executed. */
export async function runAssistant(
  request: AssistantRequest,
  onEvent: (event: AgentEvent) => void,
): Promise<void> {
  const channel = new Channel<AgentEvent>();
  channel.onmessage = onEvent;
  await unwrap(commands.assistantRun(request, channel));
}
/** Cancel a tab-owned turn, including its provider and protected database work. */
export async function cancelAssistant(id: string): Promise<void> {
  await unwrap(commands.assistantCancel(id));
}
/** Exact gated assistant context, collected offline through the native schema producer. */
export function previewAssistant(
  request: AssistantRequest,
): Promise<ContextPreview> {
  return unwrap(commands.assistantPreview(request));
}
/** Explain a proposed SELECT using the shared protected, plan-only tool. */
export function explainAssistant(
  request: AssistantRequest,
): Promise<ContextPreview> {
  return unwrap(commands.assistantExplain(request));
}
