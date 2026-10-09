// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { Button } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import { journaledTableEdits, listProfiles } from '@openrows/ipc';
import type { JournaledEditsDto } from '@openrows/ipc';
import { openLogs } from './index';

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
    <ul aria-label="Tables with staged edits">
      {props.entries.map(({ target, rows, savedAtMs }) => (
        <li
          key={[
            target.profileId,
            target.database,
            target.schema,
            target.table,
          ].join('/')}
        >
          <strong>
            {target.schema}.{target.table}
          </strong>{' '}
          on {props.names[target.profileId] ?? 'an unknown connection'}, {rows}{' '}
          staged {rows === 1 ? 'change' : 'changes'}
          {savedAtMs === null
            ? ''
            : `, saved ${new Date(savedAtMs).toLocaleString()}`}
        </li>
      ))}
    </ul>
  );
}

function JournalSummary({ journal }: { journal: Journal }): React.JSX.Element {
  if (journal.kind === 'loading') return <p>Reading the journal…</p>;
  if (journal.kind === 'failed')
    return <p role="alert">The journal cannot be read: {journal.message}</p>;
  if (journal.entries.length === 0)
    return <p>No staged edits were left behind.</p>;
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
      title="OpenRows closed unexpectedly"
      size="alert"
      onClose={props.onClose}
    >
      <p>
        The last run did not exit cleanly. Staged table edits were kept in the
        local journal; open each table to review and restore them.
      </p>
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
            Open logs folder
          </Button>
        }
      >
        <Button variant="primary" onClick={props.onClose} data-initial-focus>
          Done
        </Button>
      </SheetActions>
    </Sheet>
  );
}
