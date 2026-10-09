// SPDX-License-Identifier: Apache-2.0
import { useMemo } from 'react';
import type { EditorDialect } from '@openrows/editor';
import { paramNames } from '@openrows/editor/params';
import type { EditorSettings } from './editorSettings';
import { ParamsBar } from './ParamsBar';
import { effectiveParams } from './queryParams';
import type { RunState } from './run';
import type { QueryTab } from './tabs';
import type { QueryTabs } from './useQueryTabs';

/**
 * The tab's parameters bar while Settings ▸ Query Params is on and the SQL
 * has parameters. Names the core asked for but the editor did not find
 * (engine-specific syntax) are listed too, so every value can be entered.
 */
export function TabParams(props: {
  tab: QueryTab;
  dialect: EditorDialect;
  settings: EditorSettings;
  run: RunState;
  tabs: Pick<QueryTabs, 'setParams'>;
  onRun: () => void;
}): React.JSX.Element | null {
  const { tab, dialect } = props;
  const style = effectiveParams(props.settings.queryParams, dialect);
  const found = useMemo(
    () => (style ? paramNames(tab.sql, dialect, style) : []),
    [tab.sql, dialect, style],
  );
  const asking = props.run.status === 'parameters';
  const asked = asking ? (props.run.parameters?.names ?? []) : [];
  // The core numbers anonymous `?` per run, the bar per document.
  const names =
    style === 'anonymous' && found.length
      ? found
      : [...new Set([...found, ...asked])];
  if (!style || !names.length) return null;
  return (
    <ParamsBar
      names={names}
      values={tab.params ?? {}}
      asking={asking}
      onChange={(values) => {
        props.tabs.setParams(tab.id, values);
      }}
      onRun={props.onRun}
    />
  );
}
