// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, SegmentedControl } from '@openrows/ui';
import { Sheet } from '@openrows/ui/sheet';
import { AppearancePane } from '../appearance/AppearancePane';
import { EditorPane } from './EditorPane';
import { GeneralPane } from './GeneralPane';
import { JsonPane } from './JsonPane';
import { ShortcutsPane } from './ShortcutsPane';
import styles from '../appearance/Appearance.module.css';

export type SettingsSection =
  'general' | 'appearance' | 'editor' | 'shortcuts' | 'json';

const SECTIONS: readonly { value: SettingsSection; label: string }[] = [
  { value: 'general', label: 'General' },
  { value: 'appearance', label: 'Appearance' },
  { value: 'editor', label: 'Editor' },
  { value: 'shortcuts', label: 'Shortcuts' },
  { value: 'json', label: 'JSON' },
];

function Pane({ section }: { section: SettingsSection }): React.JSX.Element {
  if (section === 'appearance') return <AppearancePane />;
  if (section === 'editor') return <EditorPane />;
  if (section === 'shortcuts') return <ShortcutsPane />;
  if (section === 'json') return <JsonPane />;
  return <GeneralPane />;
}

/** Settings (⌘,): every preference, saved to settings.json and applied live. */
export function SettingsSheet(props: {
  initial?: SettingsSection;
  onClose: () => void;
}): React.JSX.Element {
  const [section, setSection] = useState(props.initial ?? 'general');
  const title = SECTIONS.find((item) => item.value === section)?.label ?? '';
  return (
    <Sheet title="Settings" size="appearance" onClose={props.onClose}>
      <div className={styles.page}>
        <header className={styles.header}>
          <h1>
            <span>Settings /</span> {title}
          </h1>
          <SegmentedControl
            aria-label="Settings section"
            options={SECTIONS}
            value={section}
            onValueChange={setSection}
          />
          <Button onClick={props.onClose} data-initial-focus="true">
            Done
          </Button>
        </header>
        <Pane section={section} />
      </div>
    </Sheet>
  );
}
