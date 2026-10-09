// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense } from 'react';
import type { ImportSheetProps } from './ImportSheet';
const ImportSheet = lazy(() =>
  import('./ImportSheet').then((module) => ({ default: module.ImportSheet })),
);

/** The import sheet, loaded on first use to keep it out of the startup bundle. */
export function LazyImportSheet(props: ImportSheetProps): React.JSX.Element {
  return (
    <Suspense fallback={null}>
      <ImportSheet {...props} />
    </Suspense>
  );
}
