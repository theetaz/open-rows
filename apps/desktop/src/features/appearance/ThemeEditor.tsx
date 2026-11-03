// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, TextField, importTheme, validTheme } from '@openrows/ui';
import type { ThemeDefinition } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import { ThemeTokenEditor } from './ThemeTokenEditor';
import { ThemePreview } from './ThemePreview';
import styles from './Appearance.module.css';
import { t } from './messages';
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
    <Sheet title={t('customizeTheme')} onClose={onClose}>
      <TextField
        label={t('themeName')}
        maxLength={50}
        value={draft.name}
        onChange={(event) => {
          setDraft({ ...draft, name: event.target.value });
        }}
      />
      {(['light', 'dark'] as const).map((mode) => (
        <fieldset className={styles.paletteEditor} key={mode}>
          <legend>
            {mode === 'light' ? t('lightPalette') : t('darkPalette')}
          </legend>
          <ThemePreview theme={draft} mode={mode} />
          {(['canvas', 'accent'] as const).map((key) => (
            <label className={styles.colorField} key={key}>
              <span>{key === 'canvas' ? t('background') : t('accent')}</span>
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
      {!valid && <p role="alert">{t('keepLightBackground')}</p>}
      <SheetActions>
        <Button onClick={onClose}>{t('cancel')}</Button>
        <Button
          variant="primary"
          disabled={!valid}
          onClick={() => {
            onSave({ ...draft, name: draft.name.trim() });
          }}
        >
          {t('saveTheme')}
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
          ? t('invalidJsonPaste')
          : error instanceof Error
            ? error.message
            : t('couldNotImport'),
      );
    }
  };
  const copy = async (): Promise<void> => {
    try {
      await navigator.clipboard.writeText(text);
      setMessage(t('themeJsonCopied'));
    } catch {
      setMessage(t('selectJsonBelow'));
    }
  };
  return (
    <Sheet
      title={importing ? t('addTheme') : t('exportTheme')}
      onClose={onClose}
    >
      <p>{importing ? t('pasteOpenrowsTheme') : t('copyJsonShare')}</p>
      <label className={styles.transfer}>
        {t('themeJson')}
        <textarea
          aria-label={t('themeJson')}
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
        <Button onClick={onClose}>{importing ? t('cancel') : t('done')}</Button>
        {importing ? (
          <Button variant="primary" disabled={!text.trim()} onClick={submit}>
            {t('importTheme')}
          </Button>
        ) : (
          <Button
            variant="primary"
            onClick={() => {
              void copy();
            }}
          >
            {t('copyJson')}
          </Button>
        )}
      </SheetActions>
    </Sheet>
  );
}
