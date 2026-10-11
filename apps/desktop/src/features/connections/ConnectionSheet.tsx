// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import type { EngineDto, ProfileDto } from '@openrows/ipc';
import { ConnectionFields } from './ConnectionFields';
import { ENGINE_CHOICES, engineName, isFileEngine } from './engines';
import { EngineTile } from './EngineTile';
import { AiSharingDisclosure } from './AiSharingDisclosure';
import { EnvironmentFields } from './EnvironmentFields';
import { FileFields } from './FileFields';
import { HelperFields } from './HelperFields';
import { ProxyFields } from './ProxyFields';
import { SessionLimitsFields } from './SessionLimitsFields';
import { SshFields } from './SshFields';
import { TestResult } from './TestResult';
import { TlsFields } from './TlsFields';
import { UrlField } from './UrlField';
import { NameFields } from './ConnectionFields';
import { useConnectionForm } from './useConnectionForm';
import styles from './ConnectionSheet.module.css';

type Props = {
  profile?: ProfileDto | undefined;
  filePath?: string;
  /** The engine of a new connection (chosen in the picker). */
  engine?: EngineDto | undefined;
  onClose: () => void;
  onSaved: (profile: ProfileDto) => void;
  onDelete: (profile: ProfileDto) => void;
  onDuplicate: (profile: ProfileDto) => void;
};

/**
 * New/edit connection: URL ↔ fields, then per engine either the server,
 * proxy, SSH and TLS sections or the database file; environment, safety, test and
 * save for every engine.
 */
export function ConnectionSheet(props: Props): React.JSX.Element {
  const { profile } = props;
  const form = useConnectionForm(
    profile,
    props.onSaved,
    props.engine,
    props.filePath,
  );
  const { engine } = form.draft;
  const file = isFileEngine(engine);
  return (
    <Sheet
      title={
        profile
          ? `Edit ${profile.name}`
          : `New ${engineName(engine)} connection`
      }
      onClose={props.onClose}
    >
      <form
        className={styles.form}
        noValidate
        onSubmit={(event) => {
          event.preventDefault();
          void form.save();
        }}
      >
        <EngineRow engine={engine} />
        <UrlField url={form.url} engine={engine} />
        <hr className={styles.separator} />
        {file ? (
          <>
            <NameFields form={form} />
            <FileFields form={form} />
            <DuckdbNotice engine={engine} />
            {engine === 'libsql-file' && (
              <small className={styles.hint}>
                SQLite-compatible files only. libSQL vector columns, ALTER
                COLUMN and random rowids are unavailable locally.
              </small>
            )}
          </>
        ) : (
          <>
            <ConnectionFields form={form} />
            <HelperFields form={form} />
            <ProxyFields form={form} />
            <SshFields form={form} />
            <TlsFields form={form} />
          </>
        )}
        <SessionLimitsFields form={form} />
        <EnvironmentFields form={form} />
        <SavedSharing profile={profile} />
        <TestResult state={form.test} />
        {form.saveError && (
          <p className={styles.error} role="alert">
            {form.saveError}
          </p>
        )}
        <SheetActions
          leading={profile && <ProfileActions {...props} profile={profile} />}
        >
          <Button
            onClick={() => void form.runTest()}
            disabled={form.test.status === 'testing'}
          >
            Test
          </Button>
          <Button onClick={props.onClose}>Cancel</Button>
          <Button type="submit" variant="primary" disabled={form.saving}>
            Save
          </Button>
        </SheetActions>
      </form>
    </Sheet>
  );
}

/** The engine as a read-only line: a saved connection keeps its engine. */
function EngineRow({ engine }: { engine: EngineDto }): React.JSX.Element {
  const monogram =
    ENGINE_CHOICES.find((choice) => choice.engine === engine)?.monogram ?? '';
  return (
    <div className={styles.engineRow}>
      <span className={styles.label}>Type</span>
      <EngineTile monogram={monogram} size="small" />
      <span>{engineName(engine)}</span>
    </div>
  );
}

function ProfileActions(
  props: Props & { profile: ProfileDto },
): React.JSX.Element {
  return (
    <>
      <Button
        onClick={() => {
          props.onDelete(props.profile);
        }}
      >
        Delete…
      </Button>
      <Button
        onClick={() => {
          props.onDuplicate(props.profile);
        }}
      >
        Duplicate
      </Button>
    </>
  );
}

/** Pending DuckDB facets have an explicit reason before opening a connection. */
function DuckdbNotice({
  engine,
}: {
  engine: EngineDto;
}): React.JSX.Element | null {
  if (engine !== 'duckdb') return null;
  return (
    <small className={styles.hint}>
      Queries and cancellation are available. Catalog browsing, editing, export,
      backup and explain visualization are not available yet.
    </small>
  );
}

function SavedSharing({
  profile,
}: {
  profile: ProfileDto | undefined;
}): React.JSX.Element | null {
  return profile ? (
    <AiSharingDisclosure
      id={profile.id}
      production={profile.env.kind === 'prod'}
    />
  ) : null;
}
