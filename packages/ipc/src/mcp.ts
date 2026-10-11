// SPDX-License-Identifier: Apache-2.0
import { isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { commands } from './generated/bindings';
import type {
  CreatedToken,
  ServerStatus,
  SqlProposal,
  TokenScope,
} from './generated/bindings';
import { unwrap } from './connections';
/** Current loopback server state, credential metadata and connected clients. */
export async function mcpStatus(): Promise<ServerStatus> {
  return unwrap(commands.mcpStatus());
}
/** Create a scoped read-only token; the returned value is shown once. */
export async function createMcpToken(scope: TokenScope): Promise<CreatedToken> {
  return unwrap(commands.mcpTokenCreate(scope));
}
/** Revoke a credential and cancel in-flight work. */
export async function revokeMcpToken(id: string): Promise<void> {
  await unwrap(commands.mcpTokenRevoke(id));
}
/** Disconnect the client using this credential. */
export async function disconnectMcpClient(id: string): Promise<void> {
  await unwrap(commands.mcpClientDisconnect(id));
}
/** Drain SQL into ordinary user editor tabs, without execution. */
export async function takeMcpProposals(): Promise<SqlProposal[]> {
  return isTauri() ? unwrap(commands.mcpProposalsTake()) : [];
}
/** Notify the workspace that a model proposal is ready for editor review. */
export async function onMcpProposals(
  listener: () => void,
): Promise<() => void> {
  return isTauri() ? listen('mcp-proposal-ready', listener) : () => undefined;
}
