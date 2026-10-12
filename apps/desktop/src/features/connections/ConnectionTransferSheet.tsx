// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import { Button, SelectField } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import {
  cancelConnectionImport,
  exportConnections,
  importConnections,
  previewConnections,
} from '@openrows/ipc';
import type {
  ImportDecisionDto,
  ImportPreview,
  ImportSource,
  ProfileDto,
} from '@openrows/ipc';
import { ConnectionImportPreview } from './ConnectionImportPreview';
import styles from './ConnectionSheet.module.css';
const sources: { value: Extract<ImportSource, string>; label: string }[] = [
  { value: 'openFile', label: 'OpenRows JSON file…' },
  { value: 'pgpass', label: 'pgpass (default / PGPASSFILE)' },
  { value: 'pgService', label: 'pg_service (default / environment)' },
  { value: 'myCnf', label: 'MySQL ~/.my.cnf' },
];
type Props = {
  mode: 'import' | 'export';
  projectRoot?: string;
  profiles: readonly ProfileDto[];
  ids?: string[] | undefined;
  onClose: () => void;
  onImported: () => void;
};
/** Lazily loaded connection portability sheet: choose scope or preview and confirm. */
export default function ConnectionTransferSheet(
  props: Props,
): React.JSX.Element {
  return props.mode === 'export' ? (
    <ExportSheet {...props} />
  ) : (
    <ImportSheet {...props} />
  );
}
function ExportSheet(props: Props): React.JSX.Element {
  const [selected, setSelected] = useState(
    () => props.ids ?? props.profiles.map((profile) => profile.id),
  );
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const run = async (): Promise<void> => {
    setBusy(true);
    setError(null);
    try {
      if (await exportConnections(selected)) props.onClose();
    } catch (error) {
      setError(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  };
  const groups = [
    ...new Set(
      props.profiles.flatMap((profile) =>
        profile.group ? [profile.group] : [],
      ),
    ),
  ];
  return (
    <Sheet title="Export connections" onClose={props.onClose}>
      <div className={styles.form}>
        <p>
          Exports settings and file paths. Passwords, passphrases, private keys
          and command output are excluded.
        </p>
        <Button
          onClick={() => {
            setSelected(props.profiles.map((profile) => profile.id));
          }}
        >
          Select all
        </Button>
        <ExportSelection
          profiles={props.profiles}
          groups={groups}
          selected={selected}
          onSelected={setSelected}
        />
        {error && (
          <p role="alert" className={styles.error}>
            {error}
          </p>
        )}
      </div>
      <SheetActions>
        <Button onClick={props.onClose}>Cancel</Button>
        <Button
          variant="primary"
          disabled={busy || selected.length === 0}
          onClick={() => void run()}
        >
          Export…
        </Button>
      </SheetActions>
    </Sheet>
  );
}
function useImportPreview() {
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
  const [preview, setPreview] = useState<ImportPreview | null>(null);
  const [decisions, setDecisions] = useState<ImportDecisionDto[]>([]);
  useEffect(
    () => () => {
      if (preview)
        void cancelConnectionImport(preview.token).catch(() => undefined);
    },
    [preview],
  );
  const loaded = (value: ImportPreview | null): void => {
    if (!mounted.current) {
      if (value)
        void cancelConnectionImport(value.token).catch(() => undefined);
      return;
    }
    setPreview(value);
    setDecisions(
      value?.rows.map((row) => ({
        choice: row.conflict === 'new' ? 'keepBoth' : 'skip',
        savePassword: false,
      })) ?? [],
    );
  };
  const change = (index: number, patch: Partial<ImportDecisionDto>): void => {
    setDecisions((current) =>
      current.map((decision, at) =>
        at === index ? { ...decision, ...patch } : decision,
      ),
    );
  };
  return { preview, decisions, loaded, change };
}
function ImportSheet(props: Props): React.JSX.Element {
  const [source, setSource] =
    useState<Extract<ImportSource, string>>('openFile');
  const state = useImportPreview();
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const run = async (apply: boolean): Promise<void> => {
    setBusy(true);
    setError(null);
    try {
      if (apply && state.preview) {
        await importConnections(state.preview.token, state.decisions);
        props.onImported();
        props.onClose();
      } else
        state.loaded(
          await previewConnections(
            props.projectRoot
              ? { project: { root: props.projectRoot } }
              : source,
          ),
        );
    } catch (error) {
      setError(error instanceof Error ? error.message : String(error));
      if (apply) state.loaded(null);
    } finally {
      setBusy(false);
    }
  };
  return (
    <Sheet title="Import connections" onClose={props.onClose}>
      <div className={styles.form}>
        {!props.projectRoot && (
          <SelectField
            label="Source"
            options={sources}
            value={source}
            onValueChange={setSource}
          />
        )}
        <p>Preview first. Nothing is saved until Confirm import.</p>
        {state.preview && (
          <ConnectionImportPreview
            preview={state.preview}
            decisions={state.decisions}
            onDecision={state.change}
          />
        )}
        {error && (
          <p role="alert" className={styles.error}>
            {error}
          </p>
        )}
      </div>
      <SheetActions>
        <Button disabled={busy} onClick={props.onClose}>
          Cancel
        </Button>
        <Button disabled={busy} onClick={() => void run(false)}>
          Preview…
        </Button>
        <Button
          variant="primary"
          disabled={busy || !state.preview || state.preview.rows.length === 0}
          onClick={() => void run(true)}
        >
          Confirm import
        </Button>
      </SheetActions>
    </Sheet>
  );
}

/** Compact selection rows, shared by profile and group scopes. */
function ExportSelection(props: {
  profiles: readonly ProfileDto[];
  groups: string[];
  selected: string[];
  onSelected: (ids: string[]) => void;
}): React.JSX.Element {
  return (
    <>
      {props.groups.map((group) => (
        <Button
          key={group}
          onClick={() => {
            props.onSelected(
              props.profiles
                .filter((profile) => profile.group === group)
                .map((profile) => profile.id),
            );
          }}
        >
          Select group: {group}
        </Button>
      ))}
      {props.profiles.map((profile) => (
        <label key={profile.id}>
          <input
            type="checkbox"
            checked={props.selected.includes(profile.id)}
            onChange={(event) => {
              props.onSelected(
                event.target.checked
                  ? [...props.selected, profile.id]
                  : props.selected.filter((id) => id !== profile.id),
              );
            }}
          />
          {profile.name}
        </label>
      ))}
    </>
  );
}
