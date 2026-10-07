// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, defaultAppearance, useTheme } from '@openrows/ui';
import { Sheet } from '@openrows/ui/sheet';
import { ThemeLibrary } from './ThemeLibrary';
import { InterfaceSettings } from './InterfaceSettings';
import { TypographySettings } from './TypographySettings';
import { SchemeCards } from './ThemePreview';
import { ThemeEditor, ThemeTransfer } from './ThemeEditor';
import type { ThemeDefinition } from '@openrows/ui';
import styles from './Appearance.module.css';
type Editor =
  | { kind: 'edit'; theme: ThemeDefinition }
  | { kind: 'import' }
  | { kind: 'export'; theme: ThemeDefinition };
/** Device-local preferences, applied without unmounting the database workspace. */
export function AppearanceSettings({
  onClose,
}: {
  onClose: () => void;
}): React.JSX.Element {
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
  return (
    <Sheet title="Appearance" size="appearance" onClose={onClose}>
      <div className={styles.page}>
        <AppearanceHeader onClose={onClose} />
        <section aria-label="Color scheme">
          <h3>Color scheme</h3>
          <SchemeCards />
        </section>
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
        <InterfaceSettings />
        <TypographySettings />
        <footer className={styles.footer}>
          <p>
            Saved on this device. System accessibility preferences take
            priority.
          </p>
          <Button
            onClick={() => {
              setReset(true);
            }}
          >
            Reset appearance
          </Button>
        </footer>
      </div>
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
    </Sheet>
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

function AppearanceHeader({
  onClose,
}: {
  onClose: () => void;
}): React.JSX.Element {
  return (
    <header className={styles.header}>
      <div>
        <p className={styles.eyebrow}>Settings / Appearance</p>
        <h1>Make it yours.</h1>
        <p>Colors, clarity and type. Changes apply instantly.</p>
      </div>
      <Button onClick={onClose} data-initial-focus="true">
        Done
      </Button>
    </header>
  );
}
