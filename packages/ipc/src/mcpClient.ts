// SPDX-License-Identifier: Apache-2.0
import { commands } from './generated/bindings';
import type {
  ServerConfig,
  McpClientTool,
  McpSecretInput,
  McpServerList,
  ProjectServerView_Serialize,
  SecretEntry,
} from './generated/bindings';
import { unwrap } from './connections';
/** Public preferences and the last discovered catalogue; starts no server. */
export function listMcpServers(): Promise<McpServerList> {
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
/** Servers of every Project loaded this session; untrusted ones never start. */
export function listProjectMcpServers(): Promise<
  ProjectServerView_Serialize[]
> {
  return unwrap(commands.mcpProjectServers());
}
/** Trust the reviewed definition; `digest` must match the file as shown. */
export async function trustProjectMcpServer(
  key: string,
  digest: string,
): Promise<void> {
  await unwrap(commands.mcpProjectTrust(key, digest));
}
/** Withdraw trust and delete the server's keychain values. */
export async function revokeProjectMcpServer(key: string): Promise<void> {
  await unwrap(commands.mcpProjectRevoke(key));
}
/** Enable exactly these tools of a trusted Project server. */
export async function enableProjectMcpTools(
  key: string,
  enabled: string[],
): Promise<void> {
  await unwrap(commands.mcpProjectTools(key, enabled));
}
/** Enter a declared variable or header in the native secure dialog. */
export function promptProjectMcpSecret(
  key: string,
  name: string,
): Promise<SecretEntry> {
  return unwrap(commands.mcpProjectSecret(key, name));
}
