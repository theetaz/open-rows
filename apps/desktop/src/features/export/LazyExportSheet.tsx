// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense } from 'react';
import type { ExportSheetProps } from './ExportSheet';
const ExportSheet = lazy(() =>
  import('./ExportSheet').then((module) => ({ default: module.ExportSheet })),
);

/** The export sheet, loaded on first use to keep it out of the startup bundle. */
export function LazyExportSheet(props: ExportSheetProps): React.JSX.Element {
  return (
    <Suspense fallback={null}>
      <ExportSheet {...props} />
    </Suspense>
  );
}
