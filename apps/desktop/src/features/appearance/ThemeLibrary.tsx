// SPDX-License-Identifier: Apache-2.0
import { Button, builtInThemes, useTheme } from '@openrows/ui';
import type { ThemeDefinition } from '@openrows/ui';
import { ThemePreview } from './ThemePreview';
import styles from './Appearance.module.css';
import { t } from './messages';
type Props = {
  onEdit: (theme: ThemeDefinition) => void;
  onImport: () => void;
  onExport: (theme: ThemeDefinition) => void;
};
export function ThemeLibrary(props: Props): React.JSX.Element {
  const { appearance } = useTheme();
  const full = appearance.customThemes.length >= 24;
  return (
    <section aria-label={t('themes')}>
      <div className={styles.sectionHeader}>
        <div>
          <h3>{t('themes')}</h3>
          <p>{t('chooseSeparatePalette')}</p>
        </div>
        <div className={styles.actions}>
          <Button
            size="sm"
            disabled={full}
            onClick={() => {
              const base = builtInThemes[0];
              if (base)
                props.onEdit({
                  ...base,
                  id: `custom-${crypto.randomUUID()}`,
                  name: t('myTheme'),
                });
            }}
          >
            {t('createTheme')}
          </Button>
          <Button size="sm" disabled={full} onClick={props.onImport}>
            {t('addTheme')}
          </Button>
        </div>
      </div>
      <div className={styles.themeGrid} data-setting-row>
        {[...builtInThemes, ...appearance.customThemes].map((theme) => (
          <ThemeCard key={theme.id} theme={theme} {...props} />
        ))}
      </div>
      {full && <p role="status">{t('libraryHolds24')}</p>}
    </section>
  );
}
function ThemeCard({
  theme,
  onEdit,
  onExport,
}: Props & { theme: ThemeDefinition }): React.JSX.Element {
  const { appearance, setAppearance } = useTheme();
  const custom = theme.id.startsWith('custom-');
  const selected = [appearance.lightTheme, appearance.darkTheme].includes(
    theme.id,
  );
  const remove = (): void => {
    setAppearance({
      customThemes: appearance.customThemes.filter(
        (item) => item.id !== theme.id,
      ),
      lightTheme:
        appearance.lightTheme === theme.id ? 'openrows' : appearance.lightTheme,
      darkTheme:
        appearance.darkTheme === theme.id ? 'openrows' : appearance.darkTheme,
    });
  };
  return (
    <article className={styles.themeCard} data-selected={selected}>
      <ThemeVariants theme={theme} />
      <div className={styles.cardTitle}>
        <strong>{theme.name}</strong>
        <Button
          size="sm"
          variant="ghost"
          disabled={appearance.customThemes.length >= 24}
          aria-label={t('duplicateName', { name: theme.name })}
          onClick={() => {
            onEdit({
              ...theme,
              id: `custom-${crypto.randomUUID()}`,
              name: t('themeCopy', { name: theme.name.slice(0, 40) }),
            });
          }}
        >
          ⧉
        </Button>
      </div>
      {custom && (
        <div className={styles.cardActions}>
          <Button
            size="sm"
            onClick={() => {
              onEdit(theme);
            }}
          >
            {t('edit')}
          </Button>
          <Button
            size="sm"
            onClick={() => {
              onExport(theme);
            }}
          >
            {t('export')}
          </Button>
          <Button
            size="sm"
            aria-label={t('removeName', { name: theme.name })}
            onClick={remove}
          >
            {t('remove')}
          </Button>
        </div>
      )}
    </article>
  );
}

function ThemeVariants({
  theme,
}: {
  theme: ThemeDefinition;
}): React.JSX.Element {
  const { appearance, setAppearance } = useTheme();
  return (
    <div className={styles.variants}>
      {(['light', 'dark'] as const).map((mode) => {
        const active =
          (mode === 'light' ? appearance.lightTheme : appearance.darkTheme) ===
          theme.id;
        return (
          <button
            key={mode}
            className={styles.variant}
            aria-label={`${theme.name} ${mode}`}
            aria-pressed={active}
            onClick={() => {
              setAppearance(
                mode === 'light'
                  ? { lightTheme: theme.id }
                  : { darkTheme: theme.id },
              );
            }}
          >
            <ThemePreview theme={theme} mode={mode} />
            <span>
              {mode === 'light'
                ? active
                  ? t('light')
                  : t('light2')
                : active
                  ? t('dark')
                  : t('dark2')}
            </span>
          </button>
        );
      })}
    </div>
  );
}
