// SPDX-License-Identifier: Apache-2.0
import { commands } from './generated/bindings';
import type { ProfileInput } from './generated/bindings';
import { unwrap } from './connections';
/** Test the exact draft invocation with a native trust confirmation; no secret is returned. */
export async function testCredential(
  profileId: string | null,
  input: ProfileInput,
): Promise<void> {
  await unwrap(commands.credentialTest(profileId, input));
}
/** Cancel the current credential attempt for this profile. */
export async function cancelCredential(profileId: string): Promise<void> {
  await unwrap(commands.credentialCancel(profileId));
}
