// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { QueryEditor } from './QueryEditor';
import { createCursorStore } from './cursor';
import { useEditorSettings } from './editorSettings';
import { TabStrip } from './TabStrip';
import type { QueryTabs } from './useQueryTabs';
import styles from './Query.module.css';

const scope = { connectionId: 'local-files', database: 'Files' };

/** Detached SQL document tabs have no execution handler or session. */
export function LocalFileTabs({
  tabs,
}: {
  tabs: QueryTabs;
}): React.JSX.Element {
  const tab = tabs.activeFor(scope);
  const [settings] = useEditorSettings();
  const [cursor] = useState(createCursorStore);
  return (
    <main className={styles.content}>
      <TabStrip
        tabs={tabs}
        list={tabs.tabsFor(scope)}
        active={tab?.id ?? ''}
        connection="Local SQL files"
        onAdd={() => {
          tabs.open(scope);
        }}
      />
      <p role="status">
        Local SQL document. Select a database and open the file there to run SQL
        explicitly.
      </p>
      {tab && (
        <QueryEditor
          key={tab.id}
          doc={tab.sql}
          label={tab.title}
          dialect="postgres"
          settings={settings}
          catalog={null}
          cursor={cursor}
          onChange={(sql) => {
            tabs.edit(tab.id, sql);
          }}
          onKey={() => false}
          onReady={() => undefined}
        />
      )}
    </main>
  );
}
