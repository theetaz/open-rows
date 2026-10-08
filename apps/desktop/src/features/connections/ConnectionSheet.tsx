// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import type { ProfileDto } from '@openrows/ipc';
import { ConnectionFields } from './ConnectionFields';
import { EnvironmentFields } from './EnvironmentFields';
import { SshFields } from './SshFields';
import { TestResult } from './TestResult';
import { TlsFields } from './TlsFields';
import { UrlField } from './UrlField';
import { useConnectionForm } from './useConnectionForm';
import styles from './ConnectionSheet.module.css';

type Props = {
  profile?: ProfileDto | undefined;
  onClose: () => void;
  onSaved: (profile: ProfileDto) => void;
  onDelete: (profile: ProfileDto) => void;
  onDuplicate: (profile: ProfileDto) => void;
};

/** New/edit PostgreSQL connection: URL ↔ fields, SSH, TLS, environment, safety, test and save. */
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
        <UrlField url={form.url} />
        <hr className={styles.separator} />
        <ConnectionFields form={form} />
        <SshFields form={form} />
        <TlsFields form={form} />
        <EnvironmentFields form={form} />
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
