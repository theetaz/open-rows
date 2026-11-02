// SPDX-License-Identifier: Apache-2.0
import { Channel } from '@tauri-apps/api/core';
import { commands } from './generated/bindings';
import type { CodexStatus, Verification } from './generated/bindings';
import { unwrap } from './connections';
/** Store an API key in the OS keychain; no key is returned. */
export async function setAiKey(id: string, key: string): Promise<void> {
  await unwrap(commands.aiKeySet(id, key));
}
/** Remove a provider's OS keychain entry. */
export async function deleteAiKey(id: string): Promise<void> {
  await unwrap(commands.aiKeyDelete(id));
}
/** Metadata-only health probe using natively saved settings/permission. */
export async function verifyAiProvider(id: string): Promise<Verification> {
  return unwrap(commands.aiVerify(id));
}
/**
 * Start the Codex CLI's own ChatGPT sign-in. The browser opens natively;
 * `onUrl` receives the page address once so it can be copied. Cancel with
 * {@link cancelAiVerification}.
 */
export async function signInCodex(
  id: string,
  onUrl: (url: string) => void,
): Promise<CodexStatus> {
  const channel = new Channel<string>();
  channel.onmessage = onUrl;
  return unwrap(commands.codexLogin(id, channel));
}
/** Cancel the provider's current metadata probe or Codex sign-in. */
export async function cancelAiVerification(id: string): Promise<void> {
  await unwrap(commands.aiCancel(id));
}
