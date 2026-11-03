// SPDX-License-Identifier: Apache-2.0
import { builtInThemes, useTheme } from '@openrows/ui';
import type { ThemeDefinition } from '@openrows/ui';
import type { CSSProperties } from 'react';
import styles from './Appearance.module.css';
import { t } from './messages';
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
    '--preview-ink':
      half.tokens?.text ?? (mode === 'dark' ? '#FFFFFF' : '#000000'),
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
    themes.find((item) => item.id === appearance.lightTheme) ??
    builtInThemes[0];
  const night =
    themes.find((item) => item.id === appearance.darkTheme) ?? builtInThemes[0];
  return (
    <div className={styles.schemeGrid} data-setting-row>
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
              ? t('system')
              : value === 'light'
                ? t('light3')
                : t('dark3')}
            {mode === value && <b aria-hidden="true">✓</b>}
          </span>
        </button>
      ))}
    </div>
  );
}
