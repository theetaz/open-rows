// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense } from 'react';
import type { Connections } from '../connections';
import type { QueryTabs } from '../query';
import { updateIsSafe } from '../settings/updateSafety';
const UpdateNotice = lazy(() =>
  import('../settings/UpdateNotice').then((module) => ({
    default: module.UpdateNotice,
  })),
);

/** Keep the updater lazy and require tab-close guards and sessions to finish. */
export function WorkspaceUpdates({
  connections,
  tabs,
}: {
  connections: Connections;
  tabs: QueryTabs;
}): React.JSX.Element {
  const states = connections.profiles.map(
    (profile) => connections.statusOf(profile.id).state,
  );
  const canInstall = updateIsSafe(tabs.state.tabs.length, states);
  return (
    <Suspense fallback={null}>
      <UpdateNotice canInstall={canInstall} />
    </Suspense>
  );
}
