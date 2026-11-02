// SPDX-License-Identifier: Apache-2.0
/**
 * The mounted editor text of query tabs. The editor hands its text to the
 * tab state after a short pause in typing, so code that copies a tab right
 * away (moving it to another window) reads the newest text here.
 */
const editors = new Map<string, () => string>();

/** Register (`read`) or forget (`null`) the mounted editor of tab `id`. */
export function trackLiveText(id: string, read: (() => string) | null): void {
  if (read) editors.set(id, read);
  else editors.delete(id);
}

/** The newest text of tab `id`'s mounted editor; `undefined` when not mounted. */
export function liveText(id: string): string | undefined {
  return editors.get(id)?.();
}
