// SPDX-License-Identifier: Apache-2.0
/** What went wrong with a user MCP server, and what to do about it. */
export function mcpFailure(error: unknown, server: string): string {
  const kind =
    error && typeof error === 'object' && 'kind' in error
      ? String(error.kind)
      : '';
  switch (kind) {
    case 'timeout':
      return `${server} did not answer within 5 seconds. Check that it starts quickly and speaks MCP.`;
    case 'network':
      return `Could not start or reach ${server}. Check the executable path or URL, and that the server runs on its own.`;
    case 'protocol':
      return `${server} returned more than OpenRows accepts (at most 64 tools, 8 KiB per tool schema, 64 KiB per result).`;
    case 'invalid':
      return 'These settings are not valid. Use an executable path, or an https:// URL (http:// only for localhost) with no user, password, query or fragment. Put secrets in environment variables or headers as a JSON object of strings.';
    case 'secret':
    case 'auth':
      return 'The OS keychain refused access to this server’s secrets. Allow OpenRows in the keychain prompt and try again.';
    case 'unsupported':
      return 'Turn on AI features in Settings → AI before discovering tools.';
    case 'cancelled':
      return 'Cancelled because MCP settings or credentials changed meanwhile. Try again.';
    case 'locked':
      return 'OpenRows is locked. Unlock it and try again.';
    case 'storage':
      return 'Could not write the MCP server list to disk.';
    default:
      return `${server}: the MCP operation failed.`;
  }
}
