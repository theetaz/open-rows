// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import {
  Button,
  builtInThemes,
  defaultAppearance,
  useTheme,
} from '@openrows/ui';
import { Sheet } from '@openrows/ui/sheet';
import { ThemeLibrary } from './ThemeLibrary';
import { InterfaceSettings } from './InterfaceSettings';
import { TypographySettings } from './TypographySettings';
import { SchemeCards } from './ThemePreview';
import { ThemeEditor, ThemeTransfer } from './ThemeEditor';
import type { ThemeDefinition } from '@openrows/ui';
import { useSearching, useSettingMatch } from '../settings/settingsSearch';
import styles from './Appearance.module.css';
import { t } from './messages';
type Editor =
  | { kind: 'edit'; theme: ThemeDefinition }
  | { kind: 'import' }
  | { kind: 'export'; theme: ThemeDefinition };
/** Settings ▸ Appearance: schemes, themes, interface and typography, applied live. */
export function AppearancePane(): React.JSX.Element {
  const { appearance, setAppearance } = useTheme();
  const [editor, setEditor] = useState<Editor | null>(null);
  const [reset, setReset] = useState(false);
  const save = (theme: ThemeDefinition): void => {
    setAppearance({
      customThemes: [
        ...appearance.customThemes.filter((item) => item.id !== theme.id),
        theme,
      ],
      ...(appearance.customThemes.some((item) => item.id === theme.id)
        ? {}
        : { lightTheme: theme.id, darkTheme: theme.id }),
    });
    setEditor(null);
  };
  const closeReset = (): void => {
    setReset(false);
  };
  const searching = useSearching();
  const scheme = useSettingMatch(t('colorScheme'), t('systemLightDark'));
  const library = useSettingMatch(
    t('themesPaletteLight'),
    ...[...builtInThemes, ...appearance.customThemes].map((item) => item.name),
  );
  return (
    <>
      {scheme && (
        <section aria-label={t('colorScheme')}>
          <h3>{t('colorScheme')}</h3>
          <SchemeCards />
        </section>
      )}
      {library && (
        <ThemeLibrary
          onEdit={(theme) => {
            setEditor({ kind: 'edit', theme });
          }}
          onImport={() => {
            setEditor({ kind: 'import' });
          }}
          onExport={(theme) => {
            setEditor({ kind: 'export', theme });
          }}
        />
      )}
      <InterfaceSettings />
      <TypographySettings />
      {!searching && (
        <AppearanceFooter
          onReset={() => {
            setReset(true);
          }}
        />
      )}
      {editor?.kind === 'edit' && (
        <ThemeEditor
          theme={editor.theme}
          onSave={save}
          onClose={() => {
            setEditor(null);
          }}
        />
      )}
      {editor && editor.kind !== 'edit' && (
        <ThemeTransfer
          theme={editor.kind === 'export' ? editor.theme : undefined}
          onSave={save}
          onClose={() => {
            setEditor(null);
          }}
        />
      )}
      {reset && <ResetAppearance onClose={closeReset} />}
    </>
  );
}

/** Where appearance is saved, and Reset appearance. */
function AppearanceFooter(props: { onReset: () => void }): React.JSX.Element {
  return (
    <footer className={styles.footer}>
      <p>{t('savedSettingsJson')}</p>
      <Button onClick={props.onReset}>{t('resetAppearance')}</Button>
    </footer>
  );
}

function ResetAppearance({
  onClose,
}: {
  onClose: () => void;
}): React.JSX.Element {
  const { appearance, setAppearance, setMode } = useTheme();
  return (
    <Sheet title={t('resetAppearance2')} size="alert" onClose={onClose}>
      <p>{t('restoreDefaultColors')}</p>
      <div className={styles.actions}>
        <Button onClick={onClose}>{t('cancel')}</Button>
        <Button
          variant="primary"
          onClick={() => {
            setAppearance({
              ...defaultAppearance,
              customThemes: appearance.customThemes,
            });
            setMode('system');
            onClose();
          }}
        >
          {t('restoreDefaults')}
        </Button>
      </div>
    </Sheet>
  );
}
