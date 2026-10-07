// SPDX-License-Identifier: Apache-2.0
import { builtInThemes, useTheme } from '@openrows/ui';
import type { ThemeDefinition } from '@openrows/ui';
import type { CSSProperties } from 'react';
import styles from './Appearance.module.css';
/** A miniature database layout, rather than a screenshot of a different product. */
export function ThemePreview({
  theme,
  mode,
}: {
  theme: ThemeDefinition;
  mode: 'light' | 'dark';
}): React.JSX.Element {
  const half = theme[mode];
  const style = {
    '--preview-canvas': half.canvas,
    '--preview-accent': half.accent,
  } as CSSProperties;
  return (
    <div
      className={styles.preview}
      style={style}
      data-mode={mode}
      aria-hidden="true"
    >
      <div className={styles.previewRail}>
        <i />
        <i />
        <i />
      </div>
      <div className={styles.previewTable}>
        <span />
        <div />
        <div />
        <div />
        <div />
        <div />
      </div>
      <div className={styles.previewInspector}>
        <i />
        <i />
        <i />
      </div>
    </div>
  );
}
export function SchemeCards(): React.JSX.Element {
  const { mode, setMode, appearance } = useTheme();
  const themes = [...builtInThemes, ...appearance.customThemes];
  const day =
    themes.find((t) => t.id === appearance.lightTheme) ?? builtInThemes[0];
  const night =
    themes.find((t) => t.id === appearance.darkTheme) ?? builtInThemes[0];
  return (
    <div className={styles.schemeGrid}>
      {(['system', 'light', 'dark'] as const).map((value) => (
        <button
          key={value}
          className={styles.scheme}
          aria-pressed={mode === value}
          onClick={() => {
            setMode(value);
          }}
        >
          {value === 'system' ? (
            <div className={styles.splitPreview}>
              {day && <ThemePreview theme={day} mode="light" />}
              {night && <ThemePreview theme={night} mode="dark" />}
            </div>
          ) : (
            (value === 'light' ? day : night) && (
              <ThemePreview
                theme={(value === 'light' ? day : night) as ThemeDefinition}
                mode={value}
              />
            )
          )}
          <span>
            {value === 'system'
              ? 'System'
              : value === 'light'
                ? 'Light'
                : 'Dark'}
            {mode === value && <b aria-hidden="true">✓</b>}
          </span>
        </button>
      ))}
    </div>
  );
}
