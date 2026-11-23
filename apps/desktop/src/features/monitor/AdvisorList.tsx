// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense, useState } from 'react';
import { Button } from '@openrows/ui';
import { proposeMonitorSql } from '@openrows/ipc';
import type { MonitorTarget, Suggestion } from '@openrows/ipc';
import { useAdvisorPreferences } from './advisorPreferences';
import styles from './Monitor.module.css';
import parts from './MonitorParts.module.css';
import { rich, t } from './messages/lazyInvestigation';
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
        setError(t('evidenceChangedEditor'));
      },
    );
  };
  return (
    <section aria-label={t('optimisationAdvisor')} className={parts.advisor}>
      <h3>{t('optimisationAdvisor')}</h3>
      <p className={styles.note}>{t('proposalsOpenEditor')}</p>
      {props.production && (
        <p role="status" className={styles.notice} data-tone="warning">
          {t('productionReviewLocks')}
        </p>
      )}
      {!cards.length && <p>{t('noSuggestionsSufficient')}</p>}
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
      <Button size="sm" variant="ghost" onClick={preferences.reset}>
        {t('restoreDismissedSuggestions')}
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
    <ul aria-label={t('advisorSuggestions')}>
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

function CardEvidence({ card }: { card: Suggestion }): React.JSX.Element {
  return (
    <details className={styles.note}>
      <summary>{t('evidenceTradeOffs')}</summary>
      <p>{card.evidence}</p>
      <p>{card.tradeOffs}</p>
      <p>
        {t('collectedValue', {
          value:
            card.collectedMs === null
              ? t('unknown')
              : new Date(card.collectedMs).toLocaleString(),
        })}
      </p>
    </details>
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
    <li className={parts.suggestion}>
      {rich('strongConfidenceConfidence', {
        strong: (
          <strong>
            {t('ruleidVversion', {
              ruleId: card.ruleId,
              version: String(card.version),
            })}
          </strong>
        ),
        confidence: card.confidence,
      })}
      <p>{card.impact}</p>
      <CardEvidence card={card} />
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
          {t('viewEvidence')}
        </Button>
        {card.sql && (
          <Button
            size="sm"
            onClick={() => {
              props.onOpen();
            }}
          >
            {props.opened ? t('openedEditor') : t('openSqlEditor')}
          </Button>
        )}
        {props.onWhatIf && (
          <Button size="sm" onClick={props.onWhatIf}>
            {t('hypopgWhatIf')}
          </Button>
        )}
        <span className={styles.spacer} />
        <Button
          size="sm"
          variant="ghost"
          onClick={() => {
            props.onDismiss();
          }}
        >
          {t('dismiss')}
        </Button>
        <Button
          size="sm"
          variant="ghost"
          onClick={() => {
            props.onSnooze();
          }}
        >
          {t('snooze1Hour')}
        </Button>
      </div>
    </li>
  );
}
