// SPDX-License-Identifier: Apache-2.0
import { Button, StatusDot } from '@openrows/ui';
import { Sheet } from '@openrows/ui/sheet';
import type { ProfileDto } from '@openrows/ipc';
import { ConnectionFields } from './ConnectionFields';
import { EnvironmentFields } from './EnvironmentFields';
import { serverLabel } from './sessions';
import { UrlField } from './UrlField';
import { useConnectionForm } from './useConnectionForm';
import type { TestState } from './useConnectionForm';
import styles from './ConnectionSheet.module.css';

type Props = {
  profile?: ProfileDto | undefined;
  onClose: () => void;
  onSaved: (profile: ProfileDto) => void;
  onDelete: (profile: ProfileDto) => void;
  onDuplicate: (profile: ProfileDto) => void;
};

/** New/edit PostgreSQL connection: URL fill, fields, environment, safety, test and save. */
export function ConnectionSheet(props: Props): React.JSX.Element {
  const { profile } = props;
  const form = useConnectionForm(profile, props.onSaved);
  return (
    <Sheet
      title={profile ? `Edit ${profile.name}` : 'New connection'}
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
        <UrlField error={form.urlError} onFill={form.pasteUrl} />
        <ConnectionFields form={form} />
        <EnvironmentFields form={form} />
        <TestResult state={form.test} />
        {form.saveError && (
          <p className={styles.error} role="alert">
            {form.saveError}
          </p>
        )}
        <footer className={styles.footer}>
          {profile && <ProfileActions {...props} profile={profile} />}
          <span className={styles.spacer} />
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
        </footer>
      </form>
    </Sheet>
  );
}

function ProfileActions(
  props: Props & { profile: ProfileDto },
): React.JSX.Element {
  return (
    <>
      <Button
        variant="destructive"
        onClick={() => {
          props.onDelete(props.profile);
        }}
      >
        Delete…
      </Button>
      <Button
        variant="ghost"
        onClick={() => {
          props.onDuplicate(props.profile);
        }}
      >
        Duplicate
      </Button>
    </>
  );
}

function TestResult({ state }: { state: TestState }): React.JSX.Element {
  switch (state.status) {
    case 'idle':
      return <div role="status" className={styles.test} />;
    case 'testing':
      return (
        <div role="status" className={styles.test}>
          <StatusDot tone="info" label="Testing connection…" />
        </div>
      );
    case 'ok':
      return (
        <div role="status" className={styles.test}>
          <StatusDot
            tone="success"
            label={`Connected · ${serverLabel(state.report.server)} · ${String(state.report.elapsedMs)} ms`}
          />
        </div>
      );
    case 'error':
      return (
        <div role="alert" className={styles.test}>
          <StatusDot tone="danger" label="Connection failed" />
          <p className={styles.error}>{state.message}</p>
        </div>
      );
  }
}
