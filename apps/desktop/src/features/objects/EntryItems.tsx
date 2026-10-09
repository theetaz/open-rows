// SPDX-License-Identifier: Apache-2.0
import type { EditorEntry } from './menus';
import type { ObjectEditorRequest } from './requests';

/**
 * Menu items that open object editors; entries the engine cannot do are
 * shown disabled with the reason as their tooltip.
 */
export function EntryItems({
  entries,
  onChoose,
}: {
  entries: readonly EditorEntry[];
  onChoose: (request: ObjectEditorRequest) => void;
}): React.JSX.Element {
  return (
    <>
      {entries.map((entry) => (
        <button
          key={entry.label}
          type="button"
          role="menuitem"
          disabled={!entry.request}
          title={entry.reason}
          data-danger={entry.danger}
          onClick={() => {
            if (entry.request) onChoose(entry.request);
          }}
        >
          {entry.label}
        </button>
      ))}
    </>
  );
}
