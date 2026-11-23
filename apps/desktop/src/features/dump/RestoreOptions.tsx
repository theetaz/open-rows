// SPDX-License-Identifier: Apache-2.0
import { SelectField, TextField } from '@openrows/ui';
import type {
  ArchiveEntryDto,
  DumpEngineDto,
  DumpFileKindDto,
  RestoreCleanDto,
  RestoreOptionsDto,
  RestoreTargetDto,
} from '@openrows/ipc';
import { isArchive } from './model';
import styles from './Dump.module.css';
import { t } from './messages';

const CLEANS: readonly { value: RestoreCleanDto; label: string }[] = [
  { value: 'keep', label: t('keepExistingObjects') },
  { value: 'dropIfExists', label: t('dropObjectsFirst') },
  { value: 'drop', label: t('dropObjectsFirstClean') },
];

const JOBS = [1, 2, 4, 8].map((jobs) => ({
  value: String(jobs),
  label:
    jobs === 1 ? t('oneTime') : t('jobsParallelJobs', { jobs: String(jobs) }),
}));

/** The session's database, or a new one created before the restore. */
export function TargetChoice(props: {
  engine: DumpEngineDto;
  database: string;
  target: RestoreTargetDto;
  onChange: (target: RestoreTargetDto) => void;
}): React.JSX.Element | null {
  if (props.engine === 'sqlite') return null;
  const name = props.target.kind === 'create' ? props.target.name : '';
  return (
    <fieldset className={styles.options}>
      <legend>{t('restoreInto')}</legend>
      <label>
        <input
          type="radio"
          name="dump-restore-target"
          checked={props.target.kind === 'existing'}
          onChange={() => {
            props.onChange({ kind: 'existing' });
          }}
        />
        {t('databaseOpenDatabase', { database: props.database })}
      </label>
      <label>
        <input
          type="radio"
          name="dump-restore-target"
          checked={props.target.kind === 'create'}
          onChange={() => {
            props.onChange({ kind: 'create', name });
          }}
        />
        {t('newDatabase')}
      </label>
      {props.target.kind === 'create' && (
        <TextField
          aria-label={t('newDatabaseName')}
          placeholder={t('name')}
          spellCheck={false}
          value={name}
          onChange={(event) => {
            props.onChange({ kind: 'create', name: event.target.value });
          }}
        />
      )}
    </fieldset>
  );
}

function ArchiveOptions(props: {
  kind: DumpFileKindDto;
  options: RestoreOptionsDto;
  onChange: (options: RestoreOptionsDto) => void;
}): React.JSX.Element {
  const { options, onChange } = props;
  return (
    <>
      <SelectField
        className={styles.row}
        label={t('existingObjects')}
        value={options.clean}
        options={CLEANS}
        onValueChange={(clean) => {
          onChange({ ...options, clean });
        }}
      />
      <SelectField
        className={styles.row}
        label={t('speed')}
        value={String(options.jobs)}
        disabled={props.kind === 'pgTar'}
        options={JOBS}
        onValueChange={(jobs) => {
          onChange({ ...options, jobs: Number(jobs) });
        }}
      />
      <label>
        <input
          type="checkbox"
          checked={options.noOwner}
          onChange={(event) => {
            onChange({ ...options, noOwner: event.target.checked });
          }}
        />
        {t('skipOwnershipNo')}
      </label>
      <label>
        <input
          type="checkbox"
          checked={options.noPrivileges}
          onChange={(event) => {
            onChange({ ...options, noPrivileges: event.target.checked });
          }}
        />
        {t('skipGrantsNo')}
      </label>
    </>
  );
}

/** Restore options of the picked file's kind. */
export function RestoreOptions(props: {
  kind: DumpFileKindDto;
  options: RestoreOptionsDto;
  onChange: (options: RestoreOptionsDto) => void;
}): React.JSX.Element | null {
  const { options, onChange } = props;
  if (props.kind === 'sqlite') return null;
  return (
    <fieldset className={styles.form}>
      <legend>{t('options')}</legend>
      {isArchive(props.kind) && <ArchiveOptions {...props} />}
      <label>
        <input
          type="checkbox"
          checked={options.stopOnError}
          onChange={(event) => {
            onChange({ ...options, stopOnError: event.target.checked });
          }}
        />
        {t('stopFirstError')}
      </label>
    </fieldset>
  );
}

/** Pick archive entries to restore; every entry is selected by default. */
export function ArchiveEntries(props: {
  entries: readonly ArchiveEntryDto[];
  selected: readonly number[] | null;
  onChange: (selected: number[] | null) => void;
}): React.JSX.Element | null {
  if (!props.entries.length) return null;
  const chosen = new Set(
    props.selected ?? props.entries.map((entry) => entry.id),
  );
  const toggle = (id: number, on: boolean): void => {
    if (on) chosen.add(id);
    else chosen.delete(id);
    const all = chosen.size === props.entries.length;
    props.onChange(
      all ? null : props.entries.map((e) => e.id).filter((e) => chosen.has(e)),
    );
  };
  return (
    <ul className={styles.entries} aria-label={t('archiveContents')}>
      {props.entries.map((entry) => (
        <li key={entry.id}>
          <label title={entry.label}>
            <input
              type="checkbox"
              checked={chosen.has(entry.id)}
              onChange={(event) => {
                toggle(entry.id, event.target.checked);
              }}
            />
            {entry.label}
          </label>
        </li>
      ))}
    </ul>
  );
}
