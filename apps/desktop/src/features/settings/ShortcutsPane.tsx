// SPDX-License-Identifier: Apache-2.0
import { useContext, useState } from 'react';
import type { KeyboardEvent } from 'react';
import { Button } from '@openrows/ui';
import {
  COMMANDS,
  comboOf,
  conflictOf,
  displayCombo,
  rebind,
  resetBindings,
  shortcutText,
  useBindings,
} from '../commands';
import type { Bindings } from '../commands/bindings';
import type { KeyCombo } from '../commands/keys';
import { matchesQuery, SettingsQuery } from './settingsSearch';
import styles from './Settings.module.css';

/** A shortcut that cannot be used yet: why, and whom it would replace. */
type Problem = { message: string; combo?: KeyCombo; steal?: string };

/** What a key press in the recorder means. */
function recorded(
  event: KeyboardEvent,
  bindings: Bindings,
  id: string,
): { combo: KeyCombo | null } | { problem: Problem } | null {
  if (['Backspace', 'Delete'].includes(event.key) && !event.metaKey)
    return { combo: null };
  const combo = comboOf(event);
  if (!combo) return null;
  if (!combo.meta && !combo.ctrl && !/^F\d+$/.test(combo.code))
    return { problem: { message: 'Use ⌘ or ⌃ with a key.' } };
  const conflict = conflictOf(bindings, id, combo);
  if (!conflict) return { combo };
  const shown = displayCombo(combo);
  if (conflict.kind === 'reserved')
    return {
      problem: { message: `${shown} is reserved for ${conflict.owner}.` },
    };
  return {
    problem: {
      message: `${shown} is used by “${conflict.owner}”.`,
      combo,
      ...(conflict.id ? { steal: conflict.id } : {}),
    },
  };
}

/** One command: its shortcut button, which records the next key press. */
function ShortcutRow(props: {
  id: string;
  label: string;
  bindings: Bindings;
}): React.JSX.Element {
  const { id, bindings } = props;
  const [recording, setRecording] = useState(false);
  const [problem, setProblem] = useState<Problem | null>(null);
  const onKey = (event: KeyboardEvent): void => {
    if (!recording || event.key === 'Tab') return;
    event.preventDefault();
    event.stopPropagation();
    if (event.key === 'Escape') {
      setRecording(false);
      return;
    }
    const result = recorded(event, bindings, id);
    if (!result) return;
    setRecording(false);
    if ('problem' in result) setProblem(result.problem);
    else {
      setProblem(null);
      rebind(id, result.combo);
    }
  };
  return (
    <div className={styles.shortcutRow} data-setting-row>
      <span>{props.label}</span>
      <button
        type="button"
        className={styles.recorder}
        aria-label={`Shortcut for ${props.label}`}
        data-recording={recording}
        onClick={() => {
          setRecording(true);
          setProblem(null);
        }}
        onBlur={() => {
          setRecording(false);
        }}
        onKeyDown={onKey}
      >
        {recording ? 'Type shortcut…' : shortcutText(bindings, id)}
      </button>
      {problem && <ProblemNote id={id} problem={problem} onDone={setProblem} />}
    </div>
  );
}

function ProblemNote(props: {
  id: string;
  problem: Problem;
  onDone: (problem: null) => void;
}): React.JSX.Element {
  const { combo, steal, message } = props.problem;
  return (
    <p className={styles.problem} role="alert">
      {message}
      {combo && steal && (
        <Button
          size="sm"
          variant="ghost"
          onClick={() => {
            rebind(props.id, combo, steal);
            props.onDone(null);
          }}
        >
          Replace
        </Button>
      )}
    </p>
  );
}

/** Settings ▸ Shortcuts: rebind every command; menus follow immediately. */
export function ShortcutsPane(): React.JSX.Element {
  const bindings = useBindings();
  const query = useContext(SettingsQuery);
  const shown = COMMANDS.filter(({ id, label }) =>
    matchesQuery(query, [label, shortcutText(bindings, id), 'shortcut']),
  );
  return (
    <section aria-label="Keyboard shortcuts">
      <header className={styles.paneHeader}>
        <h3>Keyboard shortcuts</h3>
        <Button size="sm" onClick={resetBindings}>
          Restore defaults
        </Button>
      </header>
      <p className={styles.hint}>
        Click a shortcut, then press the new keys. Delete removes it, Escape
        cancels.
      </p>
      <div className={styles.shortcuts}>
        {shown.map(({ id, label }) => (
          <ShortcutRow key={id} id={id} label={label} bindings={bindings} />
        ))}
      </div>
    </section>
  );
}
