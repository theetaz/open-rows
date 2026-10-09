// SPDX-License-Identifier: Apache-2.0
import { Button, Icon, IconButton, TextField } from '@openrows/ui';
import { useRevealedSecret } from './useRevealedSecret';
import type { SecretDraft } from './routeForm';
import styles from './ConnectionSheet.module.css';

/** Dots shown for a saved secret; its length says nothing about the value. */
const MASK = '••••••••';

type SavedSecretProps = {
  label: string;
  noun: string;
  secret: SecretDraft;
  onChange: (secret: SecretDraft) => void;
  /** Reads the saved value from the Keychain; absent when it cannot be revealed. */
  reveal?: (() => Promise<string>) | undefined;
  change: () => void;
};

const block = (event: React.ClipboardEvent): void => {
  event.preventDefault();
};

/**
 * A secret saved in the Keychain, as TablePlus shows it: dotted until the
 * eye button reads it on demand; it hides again on blur, after a timeout or
 * when the sheet closes. Change replaces it, Forget removes it on Save.
 */
export function SavedSecret(props: SavedSecretProps): React.JSX.Element {
  const read = props.reveal ?? (() => Promise.reject(new Error('Unavailable')));
  const revealed = useRevealedSecret(read);
  const shown = revealed.value !== null;
  return (
    <div
      className={styles.stack}
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget)) revealed.hide();
      }}
    >
      <TextField
        className={styles.row}
        label={props.label}
        readOnly
        type={shown ? 'text' : 'password'}
        value={revealed.value ?? MASK}
        title="Saved in Keychain"
        {...(shown ? {} : { onCopy: block, onCut: block })}
        trailing={
          props.reveal && (
            <IconButton
              size="sm"
              variant="ghost"
              aria-label={shown ? `Hide ${props.noun}` : `Reveal ${props.noun}`}
              aria-pressed={shown}
              disabled={revealed.busy}
              onClick={(event) => {
                const frame = event.currentTarget.closest('div');
                if (shown) revealed.hide();
                else
                  void revealed.show().then(() => {
                    frame?.querySelector('input')?.focus();
                  });
              }}
            >
              <Icon name={shown ? 'eyeOff' : 'eye'} />
            </IconButton>
          )
        }
      />
      {revealed.error && (
        <small className={styles.hint} role="alert">
          {revealed.error}
        </small>
      )}
      <span className={styles.hint}>
        <Button
          size="sm"
          aria-label={`Change ${props.noun}`}
          onClick={props.change}
        >
          Change…
        </Button>
        <Button
          size="sm"
          variant="ghost"
          onClick={() => {
            props.onChange({ ...props.secret, value: '', mode: 'clear' });
          }}
        >
          {`Forget saved ${props.noun}`}
        </Button>
      </span>
    </div>
  );
}
