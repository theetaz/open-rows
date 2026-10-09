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
        ...appearance.customThemes.filter((t) => t.id !== theme.id),
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
  const scheme = useSettingMatch('Color scheme', 'system light dark mode');
  const library = useSettingMatch(
    'Themes palette light dark mode create add',
    ...[...builtInThemes, ...appearance.customThemes].map((t) => t.name),
  );
  return (
    <>
      {scheme && (
        <section aria-label="Color scheme">
          <h3>Color scheme</h3>
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
      <p>
        Saved in settings.json. System accessibility preferences take priority.
      </p>
      <Button onClick={props.onReset}>Reset appearance</Button>
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
    <Sheet title="Reset appearance?" size="alert" onClose={onClose}>
      <p>
        Restore default colors, fonts and interface controls. Your custom themes
        will stay in the library.
      </p>
      <div className={styles.actions}>
        <Button onClick={onClose}>Cancel</Button>
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
          Restore defaults
        </Button>
      </div>
    </Sheet>
  );
}
