// SPDX-License-Identifier: Apache-2.0
import { commands } from './generated/bindings';
import type {
  ServerConfig,
  McpClientTool,
  McpSecretInput,
} from './generated/bindings';
import { unwrap } from './connections';
/** List public preferences without starting user executables or HTTP sessions. */
export function listMcpServers(): Promise<ServerConfig[]> {
  return unwrap(commands.mcpServersList());
}
/** Save public preferences; native code invalidates consent and cancels active turns. */
export async function saveMcpServers(servers: ServerConfig[]): Promise<void> {
  await unwrap(commands.mcpServersSave(servers));
}
/** Write-only keychain inputs; there is no secret reveal API. */
export async function setMcpServerSecrets(
  id: string,
  input: McpSecretInput,
): Promise<void> {
  await unwrap(commands.mcpServerSecrets(id, input));
}
/** Delete keychain entries and the public server reference. */
export async function deleteMcpServer(id: string): Promise<void> {
  await unwrap(commands.mcpServerDelete(id));
}
/** Explicit bounded discovery; tools are disabled until selected by the user. */
export function discoverMcpTools(id: string): Promise<McpClientTool[]> {
  return unwrap(commands.mcpServerTools(id));
}
/** Answer only a pending native consent request. */
export async function confirmMcpTool(
  id: string,
  allow: boolean,
): Promise<void> {
  await unwrap(commands.mcpToolConfirm(id, allow));
}
