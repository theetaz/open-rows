// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Icon, SegmentedControl } from '@openrows/ui';
import type { ExtraResultTab } from '../explain';
import type { ResultView } from './editorSettings';
import type { ResultMeta } from './run';
import { resultLabel } from './StackedResults';
import styles from './ResultBar.module.css';
import { t } from './lazyMessages';

type ResultBarProps = {
  results: readonly ResultMeta[];
  current: ResultMeta | undefined;
  extra: ExtraResultTab | null;
  extraShown: boolean;
  stacked: boolean;
  onChoose: (id: string | null) => void;
  /** Messages of the shown result(s); the toggle appears when there are any. */
  messages: number;
  messagesShown: boolean;
  onMessages: () => void;
  /** Tabs or stacked, offered while the run returned several results. */
  layout: { view: ResultView; onView: (view: ResultView) => void } | null;
};

const LAYOUTS: readonly { value: ResultView; label: string }[] = [
  { value: 'tabs', label: t('resultTabs') },
  { value: 'stacked', label: t('resultStacked') },
];

/**
 * Whether the messages list shows; a new run shows it again, so its errors
 * and notices are never hidden by an earlier choice.
 */
export function useMessagesShown(
  queryId: string | null,
): [boolean, () => void] {
  const [state, setState] = useState({ queryId, shown: true });
  if (state.queryId !== queryId) setState({ queryId, shown: true });
  const shown = state.queryId === queryId ? state.shown : true;
  return [
    shown,
    () => {
      setState({ queryId, shown: !shown });
    },
  ];
}

/**
 * The 36 px bar over the results: result sets, the plan and Messages in
 * one segmented track, then the Tabs / Stacked layout. It occupies a row
 * only when it has something to offer.
 */
export function ResultBar(props: ResultBarProps): React.JSX.Element | null {
  const tabs =
    (!props.stacked && props.results.length > 1) || props.extra !== null;
  if (!tabs && props.messages === 0 && !props.layout) return null;
  return (
    <div className={styles.bar}>
      {(tabs || props.messages > 0) && (
        <div className={styles.track}>
          {tabs && <ResultTabs {...props} />}
          {tabs && props.messages > 0 && (
            <span className={styles.divider} aria-hidden="true" />
          )}
          {props.messages > 0 && (
            <button
              type="button"
              className={styles.segment}
              aria-pressed={props.messagesShown}
              onClick={props.onMessages}
            >
              {t('messages')}
              <span className={styles.count}>{props.messages}</span>
            </button>
          )}
        </div>
      )}
      <span className={styles.spacer} />
      {props.layout && (
        <SegmentedControl
          size="compact"
          aria-label={t('resultLayout')}
          options={LAYOUTS}
          value={props.layout.view}
          onValueChange={props.layout.onView}
        />
      )}
    </div>
  );
}

function ResultTabs(props: ResultBarProps): React.JSX.Element {
  const { extra } = props;
  return (
    <div role="tablist" aria-label={t('resultSets')} className={styles.tabs}>
      {props.stacked ? (
        <button
          type="button"
          role="tab"
          className={styles.segment}
          aria-selected={!props.extraShown}
          onClick={() => {
            props.onChoose(null);
          }}
        >
          {t('results')}
        </button>
      ) : (
        props.results.map((result, index) => (
          <button
            key={result.id}
            type="button"
            role="tab"
            className={styles.segment}
            aria-selected={result === props.current}
            onClick={() => {
              props.onChoose(result.id);
            }}
          >
            {resultLabel(result, index)}
          </button>
        ))
      )}
      {extra && (
        <span className={styles.extra}>
          <button
            type="button"
            role="tab"
            className={styles.segment}
            aria-selected={props.extraShown}
            onClick={() => {
              props.onChoose(extra.id);
            }}
          >
            {extra.label}
          </button>
          <button
            type="button"
            className={styles.close}
            aria-label={t('closeLabel', { label: extra.label })}
            title={t('closeLabel', { label: extra.label })}
            onClick={extra.onClose}
          >
            <Icon name="close" />
          </button>
        </span>
      )}
    </div>
  );
}
