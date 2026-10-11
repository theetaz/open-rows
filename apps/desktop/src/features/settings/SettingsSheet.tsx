// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense, useState } from 'react';
import { Button, Icon, SegmentedControl, TextField } from '@openrows/ui';
import { Sheet } from '@openrows/ui/sheet';
import { AppearancePane } from '../appearance/AppearancePane';
import { DiagnosticsPane } from './DiagnosticsPane';
import { EditorPane } from './EditorPane';
import { ExecutionPane } from './ExecutionPane';
import { GeneralPane } from './GeneralPane';
import { JsonPane } from './JsonPane';
import { SettingsActions } from './SettingsActions';
import { TablesPane } from './TablesPane';
import { LocationsPane } from './LocationsPane';
import { SearchResults } from './SearchResults';
import { ShortcutsPane } from './ShortcutsPane';
import { SecurityPane } from '../security/SecurityPane';
import { ToolsPane } from './ToolsPane';
import { SnippetsPane } from '../snippets';
import { SettingsQuery } from './settingsSearch';
import styles from '../appearance/Appearance.module.css';
import searchStyles from './Settings.module.css';

const AiPane = lazy(() => import('./ai/AiPane'));

export type SettingsSection =
  | 'ai'
  | 'tables'
  | 'locations'
  | 'security'
  | 'general'
  | 'appearance'
  | 'editor'
  | 'execution'
  | 'snippets'
  | 'shortcuts'
  | 'tools'
  | 'diagnostics'
  | 'json';

const SECTIONS: readonly { value: SettingsSection; label: string }[] = [
  { value: 'security', label: 'Security' },
  { value: 'ai', label: 'AI' },
  { value: 'tables', label: 'Tables' },
  { value: 'locations', label: 'Locations' },
  { value: 'general', label: 'General' },
  { value: 'appearance', label: 'Appearance' },
  { value: 'editor', label: 'Editor' },
  { value: 'execution', label: 'Execution' },
  { value: 'snippets', label: 'Snippets' },
  { value: 'shortcuts', label: 'Keymap' },
  { value: 'tools', label: 'Tools' },
  { value: 'diagnostics', label: 'Diagnostics' },
  { value: 'json', label: 'JSON' },
];

function Pane({ section }: { section: SettingsSection }): React.JSX.Element {
  if (section === 'ai')
    return (
      <Suspense fallback={<p role="status">Loading AI settings…</p>}>
        <AiPane />
      </Suspense>
    );
  if (section === 'tables') return <TablesPane />;
  if (section === 'locations') return <LocationsPane />;
  if (section === 'security') return <SecurityPane />;
  if (section === 'appearance') return <AppearancePane />;
  if (section === 'editor') return <EditorPane />;
  if (section === 'execution') return <ExecutionPane />;
  if (section === 'snippets') return <SnippetsPane />;
  if (section === 'shortcuts') return <ShortcutsPane />;
  if (section === 'tools') return <ToolsPane />;
  if (section === 'diagnostics') return <DiagnosticsPane />;
  if (section === 'json') return <JsonPane />;
  return <GeneralPane />;
}

/** Every searchable pane (the read-only JSON view is not one), in order. */
const SEARCHED = SECTIONS.filter((item) => item.value !== 'json').map(
  (item) => ({ ...item, pane: <Pane section={item.value} /> }),
);

/**
 * Settings (⌘,): every preference, saved to settings.json and applied live.
 * Typing in the search field lists the matching rows of every pane.
 */
export function SettingsSheet(props: {
  initial?: SettingsSection;
  onClose: () => void;
}): React.JSX.Element {
  const [section, setSection] = useState(props.initial ?? 'general');
  const [query, setQuery] = useState('');
  const searching = query.trim() !== '';
  const title = searching
    ? 'Search'
    : (SECTIONS.find((item) => item.value === section)?.label ?? '');
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
            onValueChange={(next) => {
              setQuery('');
              setSection(next);
            }}
          />
          <TextField
            className={searchStyles.search}
            type="search"
            aria-label="Search settings"
            placeholder="Search"
            leadingIcon={<Icon name="search" />}
            value={query}
            onChange={(event) => {
              setQuery(event.target.value);
            }}
          />
          <Button onClick={props.onClose} data-initial-focus="true">
            Done
          </Button>
        </header>
        <SettingsActions section={section} />
        <SettingsQuery value={query}>
          {searching ? (
            <SearchResults query={query} panes={SEARCHED} />
          ) : (
            <Pane section={section} />
          )}
        </SettingsQuery>
      </div>
    </Sheet>
  );
}
