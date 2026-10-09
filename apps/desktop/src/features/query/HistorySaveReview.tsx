// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { Button, TextField } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import { createProjectQuery, historyLiterals } from '@openrows/ipc';
import type { EngineDto, LiteralRangeDto } from '@openrows/ipc';
import { useProject } from '../projects';
import { reloadProject } from '../projects/projectStore';
import { addSavedQuery } from './savedQueries';
import styles from './HistoryReview.module.css';
/** Review every literal before history is persisted as a saved query. */
export function HistorySaveReview(props: {
  sql: string;
  profileId: string;
  engine: EngineDto;
  onClose: () => void;
}): React.JSX.Element {
  const { project } = useProject();
  const [sql, setSql] = useState(props.sql);
  const [name, setName] = useState('History query');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const save = useHistorySave({
    sql,
    name,
    project,
    profileId: props.profileId,
    onClose: props.onClose,
    setBusy,
    setError,
  });
  const [ranges, setRanges] = useState<LiteralRangeDto[] | null>(null);
  useEffect(() => {
    let live = true;
    historyLiterals(sql, props.engine).then(
      (found) => {
        if (live) setRanges(found);
      },
      (failure: unknown) => {
        if (live) setError(String(failure));
      },
    );
    return () => {
      live = false;
    };
  }, [sql, props.engine]);
  return (
    <Sheet title="Review history query" size="form" onClose={props.onClose}>
      <p>
        Review highlighted literals before saving to{' '}
        {project?.name ?? 'Local queries'}. Remove sensitive values here.
      </p>
      <LiteralPreview sql={sql} ranges={ranges ?? []} />
      <label>
        SQL to save
        <textarea
          className={styles.sql}
          aria-label="SQL to save"
          value={sql}
          onChange={(event) => {
            setSql(event.target.value);
            setRanges(null);
            setError(null);
          }}
        />
      </label>
      <TextField
        label="Query name"
        value={name}
        onChange={(event) => {
          setName(event.target.value);
          setError(null);
        }}
      />
      {error && <p role="alert">{error}</p>}
      <SheetActions>
        <Button onClick={props.onClose}>Cancel</Button>
        <Button
          variant="primary"
          disabled={busy || ranges === null || !name.trim()}
          onClick={() => {
            void save();
          }}
        >
          Save reviewed query
        </Button>
      </SheetActions>
    </Sheet>
  );
}
function LiteralPreview({
  sql,
  ranges,
}: {
  sql: string;
  ranges: readonly LiteralRangeDto[];
}): React.JSX.Element {
  const parts = ranges.map((range, index) => {
    const plain = sql.slice(ranges[index - 1]?.to ?? 0, range.from);
    return (
      <span key={range.from}>
        {plain}
        <mark>{sql.slice(range.from, range.to)}</mark>
      </span>
    );
  });
  return (
    <pre aria-label="Literal review" className={styles.preview}>
      <code>
        {parts}
        {sql.slice(ranges.at(-1)?.to ?? 0)}
      </code>
    </pre>
  );
}

/** Save only the SQL the user just reviewed, preserving errors in the sheet. */
function useHistorySave(input: {
  sql: string;
  name: string;
  profileId: string;
  project: import('@openrows/ipc').ProjectDto | null;
  onClose: () => void;
  setBusy: (busy: boolean) => void;
  setError: (error: string) => void;
}): () => Promise<void> {
  return async () => {
    input.setBusy(true);
    try {
      const { project, sql, name } = input;
      if (project) {
        const safeName = name
          .trim()
          .replace(/[/\\:]/g, '-')
          .replace(/^\.+/, '');
        await createProjectQuery(
          project.root,
          project.queryFolders[0] ?? '.',
          safeName,
          sql,
        );
        await reloadProject();
      } else await addSavedQuery(input.profileId, name.trim(), sql);
      input.onClose();
    } catch (failure) {
      input.setError(String(failure));
      input.setBusy(false);
    }
  };
}
