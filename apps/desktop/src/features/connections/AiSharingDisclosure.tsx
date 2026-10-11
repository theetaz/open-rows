// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense, useState } from 'react';
import { Button } from '@openrows/ui';
const ConnectionSharing = lazy(
  () => import('../settings/ai/ConnectionSharing'),
);
/** Load data-sharing controls only when a saved connection's owner opens them. */
export function AiSharingDisclosure(props: {
  id: string;
  production: boolean;
}): React.JSX.Element {
  const [open, setOpen] = useState(false);
  return (
    <section aria-label="AI sharing settings">
      <Button
        type="button"
        aria-expanded={open}
        onClick={() => {
          setOpen(!open);
        }}
      >
        AI data sharing
      </Button>
      {open && (
        <Suspense fallback={<p role="status">Loading sharing settings…</p>}>
          <ConnectionSharing {...props} />
        </Suspense>
      )}
    </section>
  );
}
