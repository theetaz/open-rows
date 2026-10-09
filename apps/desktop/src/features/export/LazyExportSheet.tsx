// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense } from 'react';
import type { ExportSheetProps } from './ExportSheet';
import type { TablesExportSheetProps } from './TablesExportSheet';
const ExportSheet = lazy(() =>
  import('./ExportSheet').then((module) => ({ default: module.ExportSheet })),
);
const TablesExportSheet = lazy(() =>
  import('./TablesExportSheet').then((module) => ({
    default: module.TablesExportSheet,
  })),
);

/** The export sheet, loaded on first use to keep it out of the startup bundle. */
export function LazyExportSheet(props: ExportSheetProps): React.JSX.Element {
  return (
    <Suspense fallback={null}>
      <ExportSheet {...props} />
    </Suspense>
  );
}

/** The export sheet of a schema's tables, loaded on first use. */
export function LazyTablesExportSheet(
  props: TablesExportSheetProps,
): React.JSX.Element {
  return (
    <Suspense fallback={null}>
      <TablesExportSheet {...props} />
    </Suspense>
  );
}
