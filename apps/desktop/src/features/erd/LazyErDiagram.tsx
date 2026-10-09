// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense } from 'react';
import type { ErDiagramProps } from './ErDiagram';
const ErDiagram = lazy(() =>
  import('./ErDiagram').then((module) => ({ default: module.ErDiagram })),
);

/** The ER diagram sheet, loaded (with dagre) on first use, outside the startup bundle. */
export function LazyErDiagram(props: ErDiagramProps): React.JSX.Element {
  return (
    <Suspense fallback={null}>
      <ErDiagram {...props} />
    </Suspense>
  );
}
