// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import type { EditorHandle } from './request';
import { tabFile } from './useTabFiles';
import type { TabFileActions } from './useTabFiles';
import type { QueryTab } from './tabs';
import styles from './Query.module.css';

/**
 * The tab's `.sql` file was changed by another program: Reload takes the
 * file's text, Keep keeps the editor's (Auto Save or ⌘S then writes it).
 */
export function FileChangedBar(props: {
  tab: QueryTab;
  editor: { current: EditorHandle | null };
  tabs: Pick<TabFileActions, 'bindFile' | 'edit'>;
}): React.JSX.Element | null {
  const { tab, tabs } = props;
  const file = tab.file;
  if (file?.external === undefined) return null;
  const external = file.external;
  const settle = (saved: string): void => {
    tabs.bindFile(tab.id, tabFile(file, saved, file.modifiedMs));
  };
  return (
    <div className={styles.reconnect} role="status">
      <span>{file.name} was changed by another program.</span>
      <span className={styles.reconnectActions}>
        <Button
          size="sm"
          onClick={() => {
            props.editor.current?.replace(external);
            tabs.edit(tab.id, external);
            settle(external);
          }}
        >
          Reload
        </Button>
        <Button
          size="sm"
          onClick={() => {
            settle(external);
          }}
        >
          Keep Mine
        </Button>
      </span>
    </div>
  );
}
