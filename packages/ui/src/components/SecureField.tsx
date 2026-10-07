// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import type { ClipboardEvent, InputHTMLAttributes } from 'react';
import { Icon } from '../icons/Icon';
import { IconButton } from './Button';
import { TextField } from './TextField';
import type { ControlSize } from './TextField';

type Props = Omit<
  InputHTMLAttributes<HTMLInputElement>,
  'type' | 'aria-label'
> & {
  label: string;
  error?: string;
  controlSize?: ControlSize;
};

const blockClipboard = (event: ClipboardEvent<HTMLInputElement>): void => {
  event.preventDefault();
};

/**
 * Password input: always masked until the eye button reveals what the user
 * typed. Copy and cut are blocked while masked so a secret never leaves as
 * plain text from a dotted field.
 */
export function SecureField(props: Props): React.JSX.Element {
  const [revealed, setRevealed] = useState(false);
  const hasValue = String(props.value ?? '').length > 0;
  const shown = revealed && hasValue;
  return (
    <TextField
      autoComplete="off"
      spellCheck={false}
      {...props}
      type={shown ? 'text' : 'password'}
      {...(shown ? {} : { onCopy: blockClipboard, onCut: blockClipboard })}
      trailing={
        <IconButton
          size="sm"
          variant="ghost"
          aria-label={shown ? 'Hide password' : 'Show password'}
          aria-pressed={shown}
          disabled={!hasValue || props.disabled}
          onClick={() => {
            setRevealed(!shown);
          }}
        >
          <Icon name={shown ? 'eyeOff' : 'eye'} />
        </IconButton>
      }
    />
  );
}
