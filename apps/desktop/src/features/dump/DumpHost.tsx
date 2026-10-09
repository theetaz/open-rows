// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense } from 'react';
import { useDumpView } from './dumpStore';
const BackupSheet = lazy(() =>
  import('./BackupSheet').then((module) => ({ default: module.BackupSheet })),
);
const RestoreSheet = lazy(() =>
  import('./RestoreSheet').then((module) => ({ default: module.RestoreSheet })),
);
const DumpLog = lazy(() =>
  import('./DumpLog').then((module) => ({ default: module.DumpLog })),
);

function View(): React.JSX.Element | null {
  const view = useDumpView();
  if (!view) return null;
  if (view.kind === 'backup') return <BackupSheet target={view.target} />;
  if (view.kind === 'restore')
    return (
      <RestoreSheet
        key={view.file.path}
        target={view.target}
        file={view.file}
      />
    );
  return <DumpLog key={view.id} id={view.id} />;
}

/** The backup, restore or log sheet when one is shown; jobs go on while it is closed. */
export function DumpHost(): React.JSX.Element {
  return (
    <Suspense fallback={null}>
      <View />
    </Suspense>
  );
}
