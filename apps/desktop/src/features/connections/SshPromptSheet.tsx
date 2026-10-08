// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, SecureField, TextField } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import type { SshPromptAnswer, SshPromptDto } from '@openrows/ipc';
import { useSshPrompt } from './useSshPrompt';
import styles from './ConnectionSheet.module.css';

type HostKey = Extract<SshPromptDto, { kind: 'hostKey' }>;
type Challenge = Extract<SshPromptDto, { kind: 'challenge' }>;
type Answer = (answer: SshPromptAnswer) => void;

const cancel: SshPromptAnswer = { kind: 'cancel' };

/** The open SSH prompt as a sheet; nothing while no connect is waiting. */
export function SshPrompts(): React.JSX.Element | null {
  const { current, answer } = useSshPrompt();
  if (!current) return null;
  return current.prompt.kind === 'hostKey' ? (
    <HostKeySheet key={current.id} prompt={current.prompt} answer={answer} />
  ) : (
    <ChallengeSheet key={current.id} prompt={current.prompt} answer={answer} />
  );
}

/** First connection to an SSH server: show the fingerprint, trust or cancel. */
function HostKeySheet(props: {
  prompt: HostKey;
  answer: Answer;
}): React.JSX.Element {
  const { prompt, answer } = props;
  const address =
    prompt.port === 22 ? prompt.host : `${prompt.host}:${String(prompt.port)}`;
  return (
    <Sheet
      title="Trust this SSH server?"
      size="alert"
      onClose={() => {
        answer(cancel);
      }}
    >
      <div className={styles.form}>
        <p>
          {`OpenRows has not connected to ${address} before. Compare the fingerprint with the one your administrator gave you before trusting it.`}
        </p>
        <p className={styles.fingerprint}>
          <span>{prompt.algorithm}</span>
          <code>{prompt.fingerprint}</code>
        </p>
        {prompt.otherAlgorithms.length > 0 && (
          <p className={styles.error} role="alert">
            {`This server is already known with a ${prompt.otherAlgorithms.join(', ')} key. A new key type can mean the connection is being intercepted.`}
          </p>
        )}
        <small className={styles.note}>
          Trusted keys are saved in OpenRows' own known_hosts file.
        </small>
      </div>
      <SheetActions>
        <Button
          onClick={() => {
            answer(cancel);
          }}
        >
          Cancel
        </Button>
        <Button
          variant="primary"
          onClick={() => {
            answer({ kind: 'trust' });
          }}
        >
          Trust and connect
        </Button>
      </SheetActions>
    </Sheet>
  );
}

const promptLabel = (text: string): string =>
  text.trim().replace(/:$/, '') || 'Response';

/** Keyboard-interactive questions: passwords, one-time codes, other MFA. */
function ChallengeSheet(props: {
  prompt: Challenge;
  answer: Answer;
}): React.JSX.Element {
  const { prompt, answer } = props;
  const [values, setValues] = useState(() => prompt.prompts.map(() => ''));
  const title = prompt.name.trim() || 'SSH authentication';
  return (
    <Sheet
      title={title}
      onClose={() => {
        answer(cancel);
      }}
    >
      <form
        className={styles.form}
        noValidate
        onSubmit={(event) => {
          event.preventDefault();
          answer({ kind: 'answers', values });
        }}
      >
        <p>{`${prompt.user}@${prompt.host}`}</p>
        {prompt.instructions.trim() && <p>{prompt.instructions.trim()}</p>}
        {prompt.prompts.map((question, index) => {
          const Field = question.echo ? TextField : SecureField;
          return (
            <Field
              key={`${question.text}-${String(index)}`}
              className={styles.row}
              label={promptLabel(question.text)}
              autoFocus={index === 0}
              autoComplete="one-time-code"
              value={values[index] ?? ''}
              onChange={(event) => {
                const next = [...values];
                next[index] = event.target.value;
                setValues(next);
              }}
            />
          );
        })}
        <SheetActions>
          <Button
            onClick={() => {
              answer(cancel);
            }}
          >
            Cancel
          </Button>
          <Button type="submit" variant="primary">
            Continue
          </Button>
        </SheetActions>
      </form>
    </Sheet>
  );
}
