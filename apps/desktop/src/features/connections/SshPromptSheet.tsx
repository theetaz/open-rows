// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, SecureField, TextField } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import type { SshPromptAnswer, SshPromptDto } from '@openrows/ipc';
import { useSshPrompt } from './useSshPrompt';
import styles from './ConnectionSheet.module.css';
import { t } from './messages/routes';

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

/** First connection to an SSH server: show the fingerprint; trust, connect once or cancel. */
function HostKeySheet(props: {
  prompt: HostKey;
  answer: Answer;
}): React.JSX.Element {
  const { prompt, answer } = props;
  const address =
    prompt.port === 22 ? prompt.host : `${prompt.host}:${String(prompt.port)}`;
  return (
    <Sheet
      title={t('trustSshServer')}
      size="alert"
      onClose={() => {
        answer(cancel);
      }}
    >
      <div className={styles.form}>
        <p>{t('openrowsNotConnected', { address })}</p>
        <p className={styles.fingerprint}>
          <span>{prompt.algorithm}</span>
          <code>{prompt.fingerprint}</code>
        </p>
        {prompt.otherAlgorithms.length > 0 && (
          <p className={styles.error} role="alert">
            {t('serverAlreadyKnown', {
              otherAlgorithms: prompt.otherAlgorithms.join(', '),
            })}
          </p>
        )}
        <small className={styles.note}>{t('trustedKeysSaved')}</small>
      </div>
      <SheetActions>
        <Button
          onClick={() => {
            answer(cancel);
          }}
        >
          {t('cancel')}
        </Button>
        <Button
          onClick={() => {
            answer({ kind: 'trustOnce' });
          }}
        >
          {t('connectOnce')}
        </Button>
        <Button
          variant="primary"
          onClick={() => {
            answer({ kind: 'trust' });
          }}
        >
          {t('trustConnect')}
        </Button>
      </SheetActions>
    </Sheet>
  );
}

const promptLabel = (text: string): string =>
  text.trim().replace(/:$/, '') || t('response');

/** Keyboard-interactive questions: passwords, one-time codes, other MFA. */
function ChallengeSheet(props: {
  prompt: Challenge;
  answer: Answer;
}): React.JSX.Element {
  const { prompt, answer } = props;
  const [values, setValues] = useState(() => prompt.prompts.map(() => ''));
  const title = prompt.name.trim() || t('sshAuthentication');
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
            {t('cancel')}
          </Button>
          <Button type="submit" variant="primary">
            {t('continue')}
          </Button>
        </SheetActions>
      </form>
    </Sheet>
  );
}
