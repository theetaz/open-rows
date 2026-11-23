// SPDX-License-Identifier: Apache-2.0
import { useId } from 'react';
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
import type { ConnectionForm } from './useConnectionForm';
import styles from './ConnectionSheet.module.css';
import { t } from './messages/profile';

type Props = {
  profile?: ProfileDto | undefined;
  /** Opened to change this connection's safety policy. */
  focusSafety?: boolean | undefined;
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
  return (
    <Sheet
      title={
        profile
          ? t('editName', { name: profile.name })
          : t('newEngineConnection', { engine: engineName(engine) })
      }
      size="editor"
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
        <div className={styles.columns}>
          <div className={styles.fields}>
            <SheetFields form={form} {...props} />
          </div>
          <TestColumn form={form} />
        </div>
        {form.saveError && (
          <p className={styles.error} role="alert">
            {form.saveError}
          </p>
        )}
        <FormActions form={form} {...props} />
      </form>
    </Sheet>
  );
}

/** Test (and, when editing, delete and duplicate) leading; Cancel, then Save. */
function FormActions(
  props: Props & { form: ConnectionForm },
): React.JSX.Element {
  const { form, profile } = props;
  return (
    <SheetActions
      leading={
        <>
          <Button
            onClick={() => void form.runTest()}
            disabled={form.test.status === 'testing'}
          >
            {t('test')}
          </Button>
          {profile && <ProfileActions {...props} profile={profile} />}
        </>
      }
    >
      <Button onClick={props.onClose}>{t('cancel')}</Button>
      <Button type="submit" variant="primary" disabled={form.saving}>
        {t('save')}
      </Button>
    </SheetActions>
  );
}

/** Every field of the form, top to bottom; the engine decides the middle. */
function SheetFields(
  props: Props & { form: ConnectionForm },
): React.JSX.Element {
  const { form } = props;
  const { engine } = form.draft;
  return (
    <>
      <EngineRow engine={engine} />
      <UrlField url={form.url} engine={engine} />
      <hr className={styles.separator} />
      {isFileEngine(engine) ? (
        <>
          <NameFields form={form} />
          <FileFields form={form} />
          <DuckdbNotice engine={engine} />
          {engine === 'libsql-file' && (
            <small className={styles.hint}>{t('sqliteCompatibleFiles')}</small>
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
      <hr className={styles.separator} />
      <EnvironmentFields form={form} focusSafety={props.focusSafety} />
      <SavedSharing profile={props.profile} />
    </>
  );
}

/** The Test connection steps, in a column beside the form. */
function TestColumn({ form }: { form: ConnectionForm }): React.JSX.Element {
  const id = useId();
  return (
    <aside className={styles.side} aria-labelledby={id}>
      <h3 id={id} className={styles.sideTitle}>
        {t('connectionSteps')}
      </h3>
      <div className={styles.sideBody}>
        {form.test.status === 'idle' && (
          <p className={styles.note}>{t('testShowsSteps')}</p>
        )}
        <TestResult state={form.test} />
      </div>
    </aside>
  );
}

/** The engine as a read-only line: a saved connection keeps its engine. */
function EngineRow({ engine }: { engine: EngineDto }): React.JSX.Element {
  const monogram =
    ENGINE_CHOICES.find((choice) => choice.engine === engine)?.monogram ?? '';
  return (
    <div className={styles.engineRow}>
      <span className={styles.label}>{t('type')}</span>
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
        {t('delete')}
      </Button>
      <Button
        onClick={() => {
          props.onDuplicate(props.profile);
        }}
      >
        {t('duplicate')}
      </Button>
    </>
  );
}

/** DuckDB features still pending have an explicit reason before connecting. */
function DuckdbNotice({
  engine,
}: {
  engine: EngineDto;
}): React.JSX.Element | null {
  if (engine !== 'duckdb') return null;
  return (
    <small className={styles.hint}>{t('browsingEditingTransactions')}</small>
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
