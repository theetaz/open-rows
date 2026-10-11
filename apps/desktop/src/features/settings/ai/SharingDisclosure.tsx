// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense, useState } from 'react';
import { Button } from '@openrows/ui';
const SharingPane = lazy(() => import('./SharingPane'));
/** Load connection sharing and audit UI only after an explicit settings action. */
export function SharingDisclosure(): React.JSX.Element {
  const [open, setOpen] = useState(false);
  return (
    <>
      <Button
        type="button"
        aria-expanded={open}
        onClick={() => {
          setOpen(!open);
        }}
      >
        Data sharing
      </Button>
      {open && (
        <Suspense fallback={<p role="status">Loading data sharing…</p>}>
          <SharingPane />
        </Suspense>
      )}
    </>
  );
}
