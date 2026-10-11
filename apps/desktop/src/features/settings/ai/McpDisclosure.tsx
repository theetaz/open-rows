// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense, useState } from 'react';
import { Button } from '@openrows/ui';
const McpPane = lazy(() => import('./McpPane'));
/** Load MCP credential/client controls only when requested. */
export function McpDisclosure(): React.JSX.Element {
  const [open, setOpen] = useState(false);
  return (
    <>
      <Button
        aria-expanded={open}
        onClick={() => {
          setOpen(!open);
        }}
      >
        MCP server
      </Button>
      {open && (
        <Suspense fallback={<p role="status">Loading MCP settings…</p>}>
          <McpPane />
        </Suspense>
      )}
    </>
  );
}
