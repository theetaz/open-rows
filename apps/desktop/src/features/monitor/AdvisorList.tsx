// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense, useState } from 'react';
import { Button } from '@openrows/ui';
import { proposeMonitorSql } from '@openrows/ipc';
import type { MonitorTarget, Suggestion } from '@openrows/ipc';
import { useAdvisorPreferences } from './advisorPreferences';
import styles from './Monitor.module.css';
const HypoPgReview = lazy(() =>
  import('./HypoPgReview').then((module) => ({ default: module.HypoPgReview })),
);

/** Explainable proposals with local dismissal and the normal editor execution boundary. */
export function AdvisorList(props: {
  target: MonitorTarget;
  suggestions: Suggestion[];
  now: number;
  production: boolean;
  querySql?: string | null;
  onEvidence: (link: string) => void;
}): React.JSX.Element {
  const preferences = useAdvisorPreferences(
    `${props.target.profileId}:${props.target.database}`,
  );
  const [error, setError] = useState<string | null>(null);
  const [opened, setOpened] = useState<string | null>(null);
  const [whatIf, setWhatIf] = useState<Suggestion | null>(null);
  const cards = props.suggestions.filter(
    (card) => !preferences.hidden(card.id, props.now),
  );
  const open = (card: Suggestion): void => {
    proposeMonitorSql(props.target, card.id).then(
      () => {
        setOpened(card.id);
        setError(null);
      },
      () => {
        setError(
          'Evidence changed or the editor queue is unavailable; refresh the advisor.',
        );
      },
    );
  };
  return (
    <section aria-label="Optimisation advisor" className={styles.advisor}>
      <h3>Optimisation advisor</h3>
      <p className={styles.note}>
        Proposals open an editor for review. Evidence age and coverage limit
        advice; plan costs do not measure runtime.
      </p>
      {props.production && (
        <p role="status">
          Production: review locks, I/O, storage and write cost before running
          any proposal.
        </p>
      )}
      {!cards.length && (
        <p>
          No suggestions with sufficient current evidence. Review Explain for
          scan/join evidence.
        </p>
      )}
      <SuggestionRows
        cards={cards}
        opened={opened}
        querySql={props.querySql}
        now={props.now}
        onEvidence={props.onEvidence}
        onOpen={open}
        onWhatIf={setWhatIf}
        onHide={preferences.hide}
      />
      {whatIf && props.querySql && whatIf.statementId && (
        <Suspense fallback={null}>
          <HypoPgReview
            target={props.target}
            card={whatIf}
            querySql={props.querySql}
            onClose={() => {
              setWhatIf(null);
            }}
          />
        </Suspense>
      )}
      <Button size="sm" onClick={preferences.reset}>
        Restore dismissed suggestions
      </Button>
      {(error ?? preferences.error) && (
        <p role="alert">{error ?? preferences.error}</p>
      )}
    </section>
  );
}

function SuggestionRows(props: {
  cards: Suggestion[];
  opened: string | null;
  querySql: string | null | undefined;
  now: number;
  onEvidence: (link: string) => void;
  onOpen: (card: Suggestion) => void;
  onWhatIf: (card: Suggestion) => void;
  onHide: (id: string, until: number) => void;
}): React.JSX.Element {
  return (
    <ul aria-label="Advisor suggestions">
      {props.cards.map((card) => (
        <SuggestionCard
          key={card.id}
          card={card}
          opened={props.opened === card.id}
          onEvidence={props.onEvidence}
          onOpen={() => {
            props.onOpen(card);
          }}
          onWhatIf={
            props.querySql &&
            card.statementId &&
            card.sql &&
            ['pg.selective-scan', 'pg.join-key'].includes(card.ruleId)
              ? () => {
                  props.onWhatIf(card);
                }
              : undefined
          }
          onDismiss={() => {
            props.onHide(card.id, -1);
          }}
          onSnooze={() => {
            props.onHide(card.id, props.now + 3_600_000);
          }}
        />
      ))}
    </ul>
  );
}

function SuggestionCard(props: {
  card: Suggestion;
  opened: boolean;
  onEvidence: (link: string) => void;
  onOpen: () => void;
  onWhatIf: (() => void) | undefined;
  onDismiss: () => void;
  onSnooze: () => void;
}): React.JSX.Element {
  const { card } = props;
  return (
    <li className={styles.suggestion}>
      <strong>
        {card.ruleId} v{String(card.version)}
      </strong>{' '}
      · {card.confidence} confidence
      <p>{card.impact}</p>
      <details className={styles.note}>
        <summary>Evidence and trade-offs</summary>
        <p>{card.evidence}</p>
        <p>{card.tradeOffs}</p>
        <p>
          Collected{' '}
          {card.collectedMs === null
            ? 'unknown'
            : new Date(card.collectedMs).toLocaleString()}
        </p>
      </details>
      <div className={styles.actions}>
        <Button
          size="sm"
          onClick={() => {
            props.onEvidence(
              card.statementId
                ? `statements:${card.statementId}`
                : card.evidenceLink,
            );
          }}
        >
          View evidence
        </Button>
        {card.sql && (
          <Button
            size="sm"
            onClick={() => {
              props.onOpen();
            }}
          >
            {props.opened ? 'Opened in editor' : 'Open SQL in editor'}
          </Button>
        )}
        {props.onWhatIf && (
          <Button size="sm" onClick={props.onWhatIf}>
            HypoPG what-if
          </Button>
        )}
        <Button
          size="sm"
          onClick={() => {
            props.onDismiss();
          }}
        >
          Dismiss
        </Button>
        <Button
          size="sm"
          onClick={() => {
            props.onSnooze();
          }}
        >
          Snooze 1 hour
        </Button>
      </div>
    </li>
  );
}
