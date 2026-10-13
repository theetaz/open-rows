// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense } from 'react';
import type { AssistantProps } from './context';
import { useConversation } from './conversation';
const Panel = lazy(() =>
  import('./AssistantPanel').then((module) => ({
    default: module.AssistantPanel,
  })),
);
/**
 * A per-tab dock, opened from the editor bar's AI button; the panel and
 * provider controls load only when opened.
 */
export function AssistantDock(props: AssistantProps): React.JSX.Element | null {
  const conversation = useConversation(props.tabId);
  return conversation.open ? (
    <Suspense fallback={<p>Loading assistant…</p>}>
      <Panel {...props} />
    </Suspense>
  ) : null;
}
