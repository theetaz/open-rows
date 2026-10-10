// SPDX-License-Identifier: Apache-2.0
import { themeTokenKeys } from '@openrows/ui';
import { light, dark } from '@openrows/ui/tokens';
import type { ThemeDefinition } from '@openrows/ui';
import styles from './Appearance.module.css';

/** Optional safe foreground/state tokens keep edit-state labels and glyphs intact. */
export function ThemeTokenEditor({
  draft,
  mode,
  onChange,
}: {
  draft: ThemeDefinition;
  mode: 'light' | 'dark';
  onChange: (draft: ThemeDefinition) => void;
}): React.JSX.Element {
  const palette = mode === 'light' ? light : dark;
  return (
    <details>
      <summary>Syntax and edit-state colours</summary>
      <SyntaxPreview draft={draft} mode={mode} />
      {themeTokenKeys.map((key) => (
        <label className={styles.colorField} key={key}>
          <span>{key}</span>
          <input
            aria-label={`${mode} ${key}`}
            type="color"
            value={
              draft[mode].tokens?.[key] ??
              (palette[key].startsWith('#') ? palette[key] : draft[mode].canvas)
            }
            onChange={(event) => {
              onChange({
                ...draft,
                [mode]: {
                  ...draft[mode],
                  tokens: { ...draft[mode].tokens, [key]: event.target.value },
                },
              });
            }}
          />
        </label>
      ))}
    </details>
  );
}

/** Preview syntax tokens locally without installing an unsaved theme. */
function SyntaxPreview({
  draft,
  mode,
}: {
  draft: ThemeDefinition;
  mode: 'light' | 'dark';
}): React.JSX.Element {
  const palette = mode === 'light' ? light : dark;
  return (
    <p
      style={{
        background: draft[mode].canvas,
        color: draft[mode].tokens?.text ?? palette.text,
      }}
    >
      <span
        style={{
          color:
            draft[mode].tokens?.['syntax-keyword'] ?? palette['syntax-keyword'],
        }}
      >
        SELECT
      </span>{' '}
      <span
        style={{
          color:
            draft[mode].tokens?.['syntax-string'] ?? palette['syntax-string'],
        }}
      >
        'example'
      </span>{' '}
      <span
        style={{
          color:
            draft[mode].tokens?.['syntax-comment'] ?? palette['syntax-comment'],
        }}
      >
        -- preview
      </span>
    </p>
  );
}
