// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, TextField, importTheme, validTheme } from '@openrows/ui';
import type { ThemeDefinition } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import { ThemeTokenEditor } from './ThemeTokenEditor';
import { ThemePreview } from './ThemePreview';
import styles from './Appearance.module.css';
type Props = {
  theme: ThemeDefinition;
  onSave: (theme: ThemeDefinition) => void;
  onClose: () => void;
};
/** A draft stays private until saved; previews respond while choosing colors. */
export function ThemeEditor({
  theme,
  onSave,
  onClose,
}: Props): React.JSX.Element {
  const [draft, setDraft] = useState(theme);
  const valid = validTheme(draft);
  return (
    <Sheet title="Customize theme" onClose={onClose}>
      <TextField
        label="Theme name"
        maxLength={50}
        value={draft.name}
        onChange={(event) => {
          setDraft({ ...draft, name: event.target.value });
        }}
      />
      {(['light', 'dark'] as const).map((mode) => (
        <fieldset className={styles.paletteEditor} key={mode}>
          <legend>{mode === 'light' ? 'Light palette' : 'Dark palette'}</legend>
          <ThemePreview theme={draft} mode={mode} />
          {(['canvas', 'accent'] as const).map((key) => (
            <label className={styles.colorField} key={key}>
              <span>{key === 'canvas' ? 'Background' : 'Accent'}</span>
              <input
                aria-label={`${mode} ${key}`}
                type="color"
                value={draft[mode][key]}
                onChange={(event) => {
                  setDraft({
                    ...draft,
                    [mode]: { ...draft[mode], [key]: event.target.value },
                  });
                }}
              />
              <code>{draft[mode][key]}</code>
            </label>
          ))}
          <ThemeTokenEditor draft={draft} mode={mode} onChange={setDraft} />
        </fieldset>
      ))}
      {!valid && (
        <p role="alert">
          Keep the light background bright, the dark background dark and accents
          dark enough for white button labels so text stays readable. Give your
          theme a name. Text and syntax tokens must meet WCAG AA (4.5:1) on its
          canvas and custom edit-state backgrounds.
        </p>
      )}
      <SheetActions>
        <Button onClick={onClose}>Cancel</Button>
        <Button
          variant="primary"
          disabled={!valid}
          onClick={() => {
            onSave({ ...draft, name: draft.name.trim() });
          }}
        >
          Save theme
        </Button>
      </SheetActions>
    </Sheet>
  );
}
export function ThemeTransfer({
  theme,
  onSave,
  onClose,
}: Omit<Props, 'theme'> & {
  theme: ThemeDefinition | undefined;
}): React.JSX.Element {
  const [text, setText] = useState(
    theme ? JSON.stringify({ ...theme, version: 1 }, null, 2) : '',
  );
  const [message, setMessage] = useState('');
  const importing = !theme;
  const submit = (): void => {
    try {
      onSave(importTheme(text));
    } catch (error) {
      setMessage(
        error instanceof SyntaxError
          ? 'Invalid JSON. Paste the complete theme exported from OpenRows.'
          : error instanceof Error
            ? error.message
            : 'Could not import theme.',
      );
    }
  };
  const copy = async (): Promise<void> => {
    try {
      await navigator.clipboard.writeText(text);
      setMessage('Theme JSON copied.');
    } catch {
      setMessage('Select the JSON below and copy it manually.');
    }
  };
  return (
    <Sheet title={importing ? 'Add theme' : 'Export theme'} onClose={onClose}>
      <p>
        {importing
          ? 'Paste an OpenRows theme JSON. Importing creates an independent copy.'
          : 'Copy this JSON to share your light and dark palettes.'}
      </p>
      <label className={styles.transfer}>
        Theme JSON
        <textarea
          aria-label="Theme JSON"
          value={text}
          maxLength={20000}
          readOnly={!importing}
          onChange={(event) => {
            setText(event.target.value);
            setMessage('');
          }}
          spellCheck={false}
        />
      </label>
      {message && <p role={importing ? 'alert' : 'status'}>{message}</p>}
      <SheetActions>
        <Button onClick={onClose}>{importing ? 'Cancel' : 'Done'}</Button>
        {importing ? (
          <Button variant="primary" disabled={!text.trim()} onClick={submit}>
            Import theme
          </Button>
        ) : (
          <Button
            variant="primary"
            onClick={() => {
              void copy();
            }}
          >
            Copy JSON
          </Button>
        )}
      </SheetActions>
    </Sheet>
  );
}
