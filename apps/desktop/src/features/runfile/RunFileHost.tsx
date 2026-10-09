// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense } from 'react';
import { useRunFileView } from './runFileStore';
const RunFileSetup = lazy(() =>
  import('./RunFileSetup').then((module) => ({ default: module.RunFileSetup })),
);
const RunFileProgress = lazy(() =>
  import('./RunFileProgress').then((module) => ({
    default: module.RunFileProgress,
  })),
);

/** The run-file sheet when one is shown; runs go on while it is closed. */
export function RunFileHost(): React.JSX.Element | null {
  const view = useRunFileView();
  if (!view) return null;
  return (
    <Suspense fallback={null}>
      {view.kind === 'setup' ? (
        <RunFileSetup
          key={view.file.path}
          file={view.file}
          target={view.target}
        />
      ) : (
        <RunFileProgress key={view.id} id={view.id} />
      )}
    </Suspense>
  );
}
