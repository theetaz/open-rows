// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense, useState } from 'react';
import { Button } from '@openrows/ui';
import { Sheet } from '@openrows/ui/sheet';
import type { QueryTabs } from './useQueryTabs';
const LocalFileTabs = lazy(() =>
  import('./LocalFileTabs').then((module) => ({
    default: module.LocalFileTabs,
  })),
);

/** Detached launch-time documents remain reachable after selecting a connection. */
export function NativeDocuments({
  tabs,
}: {
  tabs: QueryTabs;
}): React.JSX.Element | null {
  const [open, setOpen] = useState(false);
  if (!tabs.tabsFor({ connectionId: 'local-files', database: 'Files' }).length)
    return null;
  return (
    <>
      <Button
        size="sm"
        onClick={() => {
          setOpen(true);
        }}
      >
        Local SQL files
      </Button>
      {open && (
        <Sheet
          title="Local SQL files"
          onClose={() => {
            setOpen(false);
          }}
        >
          <Suspense fallback={null}>
            <LocalFileTabs tabs={tabs} />
          </Suspense>
        </Sheet>
      )}
    </>
  );
}
