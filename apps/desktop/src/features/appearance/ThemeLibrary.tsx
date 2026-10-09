// SPDX-License-Identifier: Apache-2.0
import { Button, builtInThemes, useTheme } from '@openrows/ui';
import type { ThemeDefinition } from '@openrows/ui';
import { ThemePreview } from './ThemePreview';
import styles from './Appearance.module.css';
type Props = {
  onEdit: (theme: ThemeDefinition) => void;
  onImport: () => void;
  onExport: (theme: ThemeDefinition) => void;
};
export function ThemeLibrary(props: Props): React.JSX.Element {
  const { appearance } = useTheme();
  const full = appearance.customThemes.length >= 24;
  return (
    <section aria-label="Themes">
      <div className={styles.sectionHeader}>
        <div>
          <h3>Themes</h3>
          <p>Choose a separate palette for light and dark mode.</p>
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
                  name: 'My theme',
                });
            }}
          >
            Create theme
          </Button>
          <Button size="sm" disabled={full} onClick={props.onImport}>
            Add theme
          </Button>
        </div>
      </div>
      <div className={styles.themeGrid} data-setting-row>
        {[...builtInThemes, ...appearance.customThemes].map((theme) => (
          <ThemeCard key={theme.id} theme={theme} {...props} />
        ))}
      </div>
      {full && (
        <p role="status">
          Your library holds 24 custom themes. Remove one before adding another.
        </p>
      )}
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
      customThemes: appearance.customThemes.filter((t) => t.id !== theme.id),
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
          aria-label={`Duplicate ${theme.name}`}
          onClick={() => {
            onEdit({
              ...theme,
              id: `custom-${crypto.randomUUID()}`,
              name: `${theme.name.slice(0, 40)} copy`,
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
            Edit
          </Button>
          <Button
            size="sm"
            onClick={() => {
              onExport(theme);
            }}
          >
            Export
          </Button>
          <Button
            size="sm"
            aria-label={`Remove ${theme.name}`}
            onClick={remove}
          >
            Remove
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
              {mode === 'light' ? 'Light' : 'Dark'} {active ? '✓' : ''}
            </span>
          </button>
        );
      })}
    </div>
  );
}
