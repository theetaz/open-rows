// SPDX-License-Identifier: Apache-2.0
import type { QueryTab } from './tabs';
import { scopeKey } from './tabs';
/** A scope's descriptor-only split layout. */
export type SplitState = {
  orientation: 'right' | 'down';
  pinned: string;
  ratio: number;
  focus: 'pinned' | 'active';
};
/** Split layout per connection/database scope. */
export type SplitStates = Record<string, SplitState>;

/** Discard corrupt splits and references to absent tabs. */
export function parseSplits(
  value: unknown,
  tabs: readonly Pick<QueryTab, 'id' | 'connectionId' | 'database'>[],
): SplitStates {
  if (typeof value !== 'object' || value === null) return {};
  const splits: SplitStates = {};
  for (const [key, item] of Object.entries(value)) {
    if (typeof item !== 'object' || item === null) continue;
    const { orientation, pinned, ratio, focus } = item as Record<
      string,
      unknown
    >;
    if (orientation !== 'right' && orientation !== 'down') continue;
    if (
      typeof pinned !== 'string' ||
      !tabs.some((tab) => tab.id === pinned && scopeKey(tab) === key)
    )
      continue;
    if (typeof ratio !== 'number' || !Number.isFinite(ratio)) continue;
    splits[key] = {
      orientation,
      pinned,
      ratio: Math.min(0.8, Math.max(0.2, ratio)),
      focus: focus === 'pinned' ? 'pinned' : 'active',
    };
  }
  return splits;
}
