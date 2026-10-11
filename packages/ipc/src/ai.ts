// SPDX-License-Identifier: Apache-2.0
import { commands } from './generated/bindings';
import type { Verification } from './generated/bindings';
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
/** Cancel the provider's current metadata probe. */
export async function cancelAiVerification(id: string): Promise<void> {
  await unwrap(commands.aiCancel(id));
}
