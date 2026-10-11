// SPDX-License-Identifier: Apache-2.0
import type { McpSettings } from '@openrows/ipc';
import { writeSettingsFile } from '@openrows/ipc';
import { localSetting } from '../localSetting';
import { settingsText } from '../settingsFile';
/** Device-local transport preferences; token values have no field in this type. */
export const MCP_KEY = 'openrows.mcp.v1';
/** Keep both transports off by default and discard unknown credential fields. */
export function parseMcpSettings(value: unknown): Required<McpSettings> {
  const fields =
    value && typeof value === 'object'
      ? (value as Record<string, unknown>)
      : {};
  const port = fields.port;
  return {
    enabled: fields.enabled === true,
    httpEnabled: fields.httpEnabled === true,
    port:
      typeof port === 'number' &&
      Number.isInteger(port) &&
      port >= 0 &&
      port <= 65535
        ? port
        : 0,
  };
}
/** Live, non-secret MCP settings shared across the lazy pane and settings file. */
export const mcpSettings = localSetting(MCP_KEY, parseMcpSettings);
/** Persist transport choices before reconciling the native server. */
export async function saveMcpSettings(next: McpSettings): Promise<void> {
  const previous = mcpSettings.get();
  mcpSettings.set(parseMcpSettings(next));
  try {
    await writeSettingsFile(settingsText());
  } catch {
    mcpSettings.set(previous);
    throw new Error('MCP settings could not be saved.');
  }
}
