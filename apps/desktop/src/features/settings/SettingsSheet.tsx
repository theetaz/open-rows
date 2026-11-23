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
import { t } from './messages/lazyGeneral';

const MonitorSettingsPane = lazy(
  () => import('../monitor/MonitorSettingsPane'),
);
const AiPane = lazy(() => import('./ai/AiPane'));

export type SettingsSection =
  | 'monitoring'
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
  { value: 'security', label: t('security') },
  { value: 'ai', label: t('ai') },
  { value: 'monitoring', label: t('monitoring') },
  { value: 'tables', label: t('tables') },
  { value: 'locations', label: t('locations') },
  { value: 'general', label: t('general') },
  { value: 'appearance', label: t('appearance') },
  { value: 'editor', label: t('editor') },
  { value: 'execution', label: t('execution') },
  { value: 'snippets', label: t('snippets') },
  { value: 'shortcuts', label: t('keymap') },
  { value: 'tools', label: t('tools') },
  { value: 'diagnostics', label: t('diagnostics') },
  { value: 'json', label: t('json') },
];

function Pane({ section }: { section: SettingsSection }): React.JSX.Element {
  if (section === 'monitoring')
    return (
      <Suspense fallback={<p>{t('loadingMonitoringSettings')}</p>}>
        <MonitorSettingsPane />
      </Suspense>
    );
  if (section === 'ai')
    return (
      <Suspense fallback={<p role="status">{t('loadingAiSettings')}</p>}>
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

/** The source list: search on top, then one row per section. */
function SettingsSidebar(props: {
  query: string;
  section: SettingsSection;
  onQuery: (query: string) => void;
  onSection: (section: SettingsSection) => void;
}): React.JSX.Element {
  return (
    <div className={searchStyles.sidebar}>
      <TextField
        className={searchStyles.search}
        type="search"
        aria-label={t('searchSettings')}
        placeholder={t('search')}
        controlSize="search"
        leadingIcon={<Icon name="search" />}
        value={props.query}
        onChange={(event) => {
          props.onQuery(event.target.value);
        }}
      />
      <SegmentedControl
        aria-label={t('settingsSection')}
        className={searchStyles.sections}
        options={SECTIONS}
        value={props.section}
        onValueChange={props.onSection}
      />
    </div>
  );
}

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
    ? t('search')
    : (SECTIONS.find((item) => item.value === section)?.label ?? '');
  return (
    <Sheet title={t('settings')} size="appearance" onClose={props.onClose}>
      <div className={searchStyles.layout}>
        <SettingsSidebar
          query={query}
          section={section}
          onQuery={setQuery}
          onSection={(next) => {
            setQuery('');
            setSection(next);
          }}
        />
        <header className={[styles.header, searchStyles.top].join(' ')}>
          <h1>
            <span>{t('settings2')}</span> {title}
          </h1>
          <Button onClick={props.onClose} data-initial-focus="true">
            {t('done')}
          </Button>
        </header>
        <div className={searchStyles.pane}>
          <div className={styles.page}>
            <SettingsActions section={section} />
            <SettingsQuery value={query}>
              {searching ? (
                <SearchResults query={query} panes={SEARCHED} />
              ) : (
                <Pane section={section} />
              )}
            </SettingsQuery>
          </div>
        </div>
      </div>
    </Sheet>
  );
}
