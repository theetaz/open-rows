// SPDX-License-Identifier: Apache-2.0
import configText from '../../../src-tauri/tauri.conf.json?raw';

type FeedConfig = {
  stable: string | null;
  beta: string | null;
  publicKey: string | null;
};
const config = (
  JSON.parse(configText) as { plugins: { openrowsUpdates: FeedConfig } }
).plugins.openrowsUpdates;

/** Select a release-owned HTTPS manifest only when a signing key is configured. */
export function updateFeed(
  channel: 'stable' | 'beta',
  feeds = config,
): string | null {
  if (!feeds.publicKey || !feeds[channel]) return null;
  try {
    const url = new URL(feeds[channel]);
    return url.protocol === 'https:' && !url.username && !url.password
      ? url.href
      : null;
  } catch {
    return null;
  }
}
