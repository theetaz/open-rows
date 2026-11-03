// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { Button } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import { journaledTableEdits, listProfiles } from '@openrows/ipc';
import type { JournaledEditsDto } from '@openrows/ipc';
import { openLogs } from './index';
import { richn, t } from './messages';

type Journal =
  | { kind: 'loading' }
  | {
      kind: 'ready';
      entries: readonly JournaledEditsDto[];
      names: Readonly<Record<string, string>>;
    }
  | { kind: 'failed'; message: string };

const messageOf = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

/** The journaled tables and the names of their connections (the journal itself stays local). */
function useJournal(): Journal {
  const [journal, setJournal] = useState<Journal>({ kind: 'loading' });
  useEffect(() => {
    let alive = true;
    Promise.all([journaledTableEdits(), listProfiles()]).then(
      ([entries, list]) => {
        if (!alive) return;
        const names = Object.fromEntries(
          list.profiles.map((profile) => [profile.id, profile.name]),
        );
        setJournal({ kind: 'ready', entries, names });
      },
      (failure: unknown) => {
        if (alive) setJournal({ kind: 'failed', message: messageOf(failure) });
      },
    );
    return () => {
      alive = false;
    };
  }, []);
  return journal;
}

function JournalList(props: {
  entries: readonly JournaledEditsDto[];
  names: Readonly<Record<string, string>>;
}): React.JSX.Element {
  return (
    <ul aria-label={t('tablesStagedEdits')}>
      {props.entries.map(({ target, rows, savedAtMs }) => (
        <li
          key={[
            target.profileId,
            target.database,
            target.schema,
            target.table,
          ].join('/')}
        >
          {savedAtMs === null
            ? richn('strongNamesCount', rows, {
                strong: (
                  <strong>
                    {target.schema}.{target.table}
                  </strong>
                ),
                names: props.names[target.profileId] ?? t('unknownConnection'),
              })
            : richn('strongNamesCountStaged', rows, {
                strong: (
                  <strong>
                    {target.schema}.{target.table}
                  </strong>
                ),
                names: props.names[target.profileId] ?? t('unknownConnection'),
                value: new Date(savedAtMs).toLocaleString(),
              })}
        </li>
      ))}
    </ul>
  );
}

function JournalSummary({ journal }: { journal: Journal }): React.JSX.Element {
  if (journal.kind === 'loading') return <p>{t('readingJournal')}</p>;
  if (journal.kind === 'failed')
    return (
      <p role="alert">{t('journalCannotRead', { message: journal.message })}</p>
    );
  if (journal.entries.length === 0) return <p>{t('noStagedEdits')}</p>;
  return <JournalList entries={journal.entries} names={journal.names} />;
}

/**
 * Shown once after a run that did not exit cleanly. The staged edits come
 * back through the table's own restore offer, so this only points at them.
 */
export function CrashRecoverySheet(props: {
  onClose: () => void;
}): React.JSX.Element {
  const journal = useJournal();
  const [failure, setFailure] = useState<string | null>(null);
  return (
    <Sheet
      title={t('openrowsClosedUnexpectedly')}
      size="alert"
      onClose={props.onClose}
    >
      <p>{t('lastRunDid')}</p>
      <JournalSummary journal={journal} />
      {failure && <p role="alert">{failure}</p>}
      <SheetActions
        leading={
          <Button
            onClick={() => {
              openLogs().catch((error: unknown) => {
                setFailure(messageOf(error));
              });
            }}
          >
            {t('openLogsFolder')}
          </Button>
        }
      >
        <Button variant="primary" onClick={props.onClose} data-initial-focus>
          {t('done')}
        </Button>
      </SheetActions>
    </Sheet>
  );
}
