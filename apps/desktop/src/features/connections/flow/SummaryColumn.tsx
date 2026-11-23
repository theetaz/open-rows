// SPDX-License-Identifier: Apache-2.0
import { EngineIcon, EnvBadge, isEngineId } from '@openrows/ui';
import type { Draft } from '../form';
import { engineName, isFileEngine } from '../engines';
import { composeUrl } from '../urlText';
import type { TestState } from '../useConnectionForm';
import { FlowIcon } from './FlowIcon';
import { summaryRows, urlParts } from './summary';
import styles from './Summary.module.css';
import page from './Flow.module.css';
import { t } from './messages';

/** The live summary: what this connection is, its URL (password masked) and status. */
export function SummaryColumn(props: {
  draft: Draft | null;
  name: string;
  test: TestState;
}): React.JSX.Element {
  const { draft } = props;
  return (
    <aside className={page.side} aria-label={t('summary')}>
      <div>
        <h2>{t('summary')}</h2>
        <SummaryCard draft={draft} name={props.name} />
      </div>
      <div>
        <h2>{t('connectionUrl')}</h2>
        <div className={styles.card}>
          <UrlText draft={draft} />
        </div>
      </div>
      <div>
        <h2>{t('status')}</h2>
        <StatusPills draft={draft} test={props.test} />
      </div>
    </aside>
  );
}

function SummaryCard(props: {
  draft: Draft | null;
  name: string;
}): React.JSX.Element {
  const { draft } = props;
  const env = draft?.envKind ?? 'local';
  return (
    <div className={styles.card}>
      <div className={styles.head}>
        {draft && isEngineId(draft.engine) ? (
          <EngineIcon engine={draft.engine} size="lg" />
        ) : (
          <span className={styles.blank}>
            <FlowIcon name="database" />
          </span>
        )}
        <span className={page.lines}>
          <b>{draft ? props.name : t('newConnection')}</b>
          <span className={page.sub}>
            {draft ? engineName(draft.engine) : t('noEngineYet')}
          </span>
        </span>
        {env === 'custom' ? (
          <span className={styles.custom}>{draft?.envLabel}</span>
        ) : (
          <EnvBadge environment={env} />
        )}
      </div>
      {draft ? (
        <dl className={styles.rows}>
          {summaryRows(draft).map(([term, value, mono]) => (
            <div key={term}>
              <dt>{term}</dt>
              <dd data-mono={mono}>{value}</dd>
            </div>
          ))}
        </dl>
      ) : (
        <p className={styles.empty}>{t('summaryEmpty')}</p>
      )}
    </div>
  );
}

function UrlText({ draft }: { draft: Draft | null }): React.JSX.Element {
  if (!draft) return <p className={styles.empty}>{t('urlEmpty')}</p>;
  const parts = urlParts(composeUrl(draft));
  return (
    <p className={styles.url}>
      {parts.map((part, index) => (
        <span key={index} data-part={part.kind}>
          {part.text}
        </span>
      ))}
    </p>
  );
}

function StatusPills(props: {
  draft: Draft | null;
  test: TestState;
}): React.JSX.Element {
  const { draft, test } = props;
  const outcome =
    test.status === 'done'
      ? test.report.server
        ? 'ok'
        : 'failed'
      : test.status;
  const label = {
    idle: t('notTested'),
    testing: t('testing'),
    ok: t('tested'),
    failed: t('testFailed'),
    error: t('testFailed'),
  }[outcome];
  const keychain =
    draft &&
    !isFileEngine(draft.engine) &&
    draft.passwordStorage === 'keychain';
  return (
    <div className={styles.pills}>
      <span className={styles.pill} data-state={outcome}>
        {outcome === 'ok' && <FlowIcon name="check" />}
        {label}
      </span>
      {keychain && (
        <span className={styles.pill}>
          <FlowIcon name="key" />
          {t('keychain')}
        </span>
      )}
      {draft?.ssh.enabled && (
        <span className={styles.pill}>
          <FlowIcon name="terminal" />
          {t('ssh')}
        </span>
      )}
    </div>
  );
}
