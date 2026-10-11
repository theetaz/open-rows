// SPDX-License-Identifier: Apache-2.0
import { commands } from './generated/bindings';
import type {
  AuditEntry,
  ContextBundle,
  ContextPreview,
  SharingPolicy,
} from './generated/bindings';
import { unwrap } from './connections';
/** Read a saved connection's device-local sharing policy; missing policies default off. */
export async function getAiSharing(
  id: string,
): Promise<Required<SharingPolicy>> {
  const policy = await unwrap(commands.aiSharingGet(id));
  return {
    level: policy.level ?? 'none',
    providers: policy.providers ?? [],
    localOnly: policy.localOnly ?? false,
    telemetry: policy.telemetry ?? false,
    productionConfirmed: policy.productionConfirmed ?? false,
    rules: policy.rules ?? [],
  };
}
/** Save explicit sharing/production consent without touching connection credentials. */
export async function saveAiSharing(
  id: string,
  policy: SharingPolicy,
): Promise<void> {
  await unwrap(commands.aiSharingSave(id, policy));
}
/** Offline preview of exact gate output; refuses unapproved context and never sends it. */
export async function previewAiContext(
  id: string,
  provider: string,
  bundle: ContextBundle,
): Promise<ContextPreview> {
  return unwrap(commands.aiContextPreview(id, provider, bundle));
}
/** Scrubbed, capped local outbound request metadata. */
export async function listAiAudit(): Promise<AuditEntry[]> {
  return unwrap(commands.aiAuditList());
}
/** Purge only local outbound request metadata. */
export async function purgeAiAudit(): Promise<void> {
  await unwrap(commands.aiAuditPurge());
}
