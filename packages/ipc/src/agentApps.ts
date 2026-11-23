// SPDX-License-Identifier: Apache-2.0
import { isTauri } from '@tauri-apps/api/core';
import { commands, events } from './generated/bindings';
import type {
  ActivityEvent,
  AgentApp,
  AppCheck,
  AppPlan,
  AppsOverview,
  ConnectRequest,
  McpActivity,
} from './generated/bindings';
import { unwrap } from './connections';

/** Find Claude Code, Codex and Gemini CLI and read local links (user action only). */
export async function listAgentApps(): Promise<AppsOverview> {
  return unwrap(commands.agentApps());
}
/** Ask the tool for its `openrows` entry; Claude and Gemini health-check it. */
export async function checkAgentApp(app: AgentApp): Promise<AppCheck> {
  return unwrap(commands.agentAppCheck(app));
}
/** The exact commands Connect will run, with a fresh client id and stable port. */
export async function planAgentApp(app: AgentApp): Promise<AppPlan> {
  return unwrap(commands.agentAppPlan(app));
}
/** Register through the tool's own `mcp add` after a confirmed plan. */
export async function connectAgentApp(request: ConnectRequest): Promise<void> {
  await unwrap(commands.agentAppConnect(request));
}
/** Run `mcp remove` and revoke the app's token; resolves a note if removal failed. */
export async function disconnectAgentApp(
  app: AgentApp,
): Promise<string | null> {
  return unwrap(commands.agentAppDisconnect(app));
}
/** Replace the app's expiring token with the same scope. */
export async function renewAgentApp(app: AgentApp): Promise<void> {
  await unwrap(commands.agentAppRenew(app));
}
/** Call `list_connections` through the registered bridge, as the app would. */
export async function testAgentApp(app: AgentApp): Promise<string> {
  return unwrap(commands.agentAppTest(app));
}
/** Recent external tool calls, newest first. */
export async function listAgentActivity(): Promise<ActivityEvent[]> {
  return isTauri() ? unwrap(commands.agentActivity()) : [];
}
/** Live external tool calls; resolves the unsubscribe function. */
export async function onAgentActivity(
  listener: (event: ActivityEvent) => void,
): Promise<() => void> {
  if (!isTauri()) return () => undefined;
  return events.mcpActivity.listen((message) => {
    const activity: McpActivity = message.payload;
    listener(activity.event);
  });
}
