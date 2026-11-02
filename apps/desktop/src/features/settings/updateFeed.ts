// SPDX-License-Identifier: Apache-2.0
import configText from '../../../src-tauri/tauri.conf.json?raw';

type FeedConfig = {
  stable: string | null;
  beta: string | null;
  publicKey: string | null;
};
const bundled = (
  JSON.parse(configText) as { plugins: { openrowsUpdates: FeedConfig } }
).plugins.openrowsUpdates;
const config: FeedConfig = {
  stable: import.meta.env.VITE_OPENROWS_UPDATE_STABLE ?? bundled.stable,
  beta: import.meta.env.VITE_OPENROWS_UPDATE_BETA ?? bundled.beta,
  publicKey: import.meta.env.VITE_OPENROWS_UPDATE_KEY ?? bundled.publicKey,
};

/** HTTPS only; development builds also accept loopback HTTP for the local update test. */
function trustedTransport(url: URL, development: boolean): boolean {
  const loopback =
    url.protocol === 'http:' && ['127.0.0.1', '[::1]'].includes(url.hostname);
  return url.protocol === 'https:' || (development && loopback);
}

/** Select a release-owned manifest only when a signing key is configured. */
export function updateFeed(
  channel: 'stable' | 'beta',
  feeds = config,
  development = import.meta.env.DEV,
): string | null {
  if (!feeds.publicKey || !feeds[channel]) return null;
  try {
    const url = new URL(feeds[channel]);
    return trustedTransport(url, development) && !url.username && !url.password
      ? url.href
      : null;
  } catch {
    return null;
  }
}
