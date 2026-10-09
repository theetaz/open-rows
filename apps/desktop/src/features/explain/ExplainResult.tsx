// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense } from 'react';
import { Button } from '@openrows/ui';
import type { ExplainControls } from './useExplain';
import styles from './Explain.module.css';
const ExplainView = lazy(() =>
  import('./ExplainView').then((module) => ({ default: module.ExplainView })),
);

/** A result tab beside the result sets (the query result area renders it). */
export type ExtraResultTab = {
  /** Changes with every new plan, so the area shows it when it arrives. */
  id: string;
  label: string;
  content: React.JSX.Element;
  onClose: () => void;
};

/** The tab showing the tab's explain, or `null` when there is none to show. */
export function explainResultTab(
  explain: ExplainControls,
): ExtraResultTab | null {
  const { state } = explain;
  if (!state || state.status === 'confirm') return null;
  const label = state.analyze ? 'Explain Analyze' : 'Query Plan';
  const base = { label, onClose: explain.close };
  switch (state.status) {
    case 'running':
      return {
        ...base,
        id: state.explainId,
        content: (
          <p className={styles.status} role="status">
            {state.analyze
              ? 'Running and measuring the statement…'
              : 'Explaining…'}
            <Button size="sm" onClick={explain.cancel}>
              Cancel
            </Button>
          </p>
        ),
      };
    case 'failed':
      return {
        ...base,
        id: `failed-${state.message}`,
        content: (
          <p className={styles.status} role="alert" data-failed="">
            {state.message}
          </p>
        ),
      };
    case 'done':
      return {
        ...base,
        id: state.explainId,
        content: (
          <Suspense fallback={null}>
            <ExplainView plan={state.plan} />
          </Suspense>
        ),
      };
  }
}
