// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense } from 'react';
import { Button } from '@openrows/ui';
import type { AssistantProps } from './context';
import { toggleAssistant, useConversation } from './conversation';
const Panel = lazy(() =>
  import('./AssistantPanel').then((module) => ({
    default: module.AssistantPanel,
  })),
);
/** A per-tab dock; the panel and provider controls load only when opened. */
export function AssistantDock(props: AssistantProps): React.JSX.Element {
  const conversation = useConversation(props.tabId);
  return conversation.open ? (
    <Suspense fallback={<p>Loading assistant…</p>}>
      <Panel {...props} />
    </Suspense>
  ) : (
    <Button
      size="sm"
      onClick={() => {
        toggleAssistant(props.tabId);
      }}
    >
      Assistant
    </Button>
  );
}
