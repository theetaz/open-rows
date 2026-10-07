// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import { Sheet } from '@openrows/ui/sheet';
import type { ProfileDto } from '@openrows/ipc';
import styles from './ConnectionSheet.module.css';

/** Explicit confirmation before a profile and its keychain password are removed. */
export function DeleteConnectionSheet(props: {
  profile: ProfileDto;
  onCancel: () => void;
  onDelete: (profile: ProfileDto) => void;
}): React.JSX.Element {
  return (
    <Sheet title="Delete connection" onClose={props.onCancel}>
      <div className={styles.form}>
        <p>
          Delete <strong>{props.profile.name}</strong>? Its saved password is
          removed from the keychain. Databases are not touched.
        </p>
        <footer className={styles.footer}>
          <span className={styles.spacer} />
          <Button onClick={props.onCancel}>Cancel</Button>
          <Button
            variant="destructive"
            onClick={() => {
              props.onDelete(props.profile);
            }}
          >
            Delete connection
          </Button>
        </footer>
      </div>
    </Sheet>
  );
}
