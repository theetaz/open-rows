// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { Icon, TextField } from '@openrows/ui';
import type { TableTabState } from './useTableTab';
import extras from './TableExtras.module.css';

/** Typing pauses this long before the server-side search runs. */
const SEARCH_DELAY_MS = 400;

/**
 * The footer's any-column search (FR-GRID-11): searches every column
 * server-side after a pause in typing, Return at once, Escape clears.
 */
export function TableSearch({
  state,
}: {
  state: TableTabState;
}): React.JSX.Element {
  const applied = state.view.search ?? '';
  const base = `${String(state.view.searchReset ?? 0)}\u0000${applied}`;
  const [draft, setDraft] = useState({ base, text: applied });
  if (draft.base !== base) setDraft({ base, text: applied });
  const text = draft.base === base ? draft.text : applied;
  const setText = (value: string): void => {
    setDraft({ base, text: value });
  };
  const { search } = state.actions;
  useEffect(() => {
    if (text === applied) return;
    const timer = window.setTimeout(() => {
      search(text);
    }, SEARCH_DELAY_MS);
    return () => {
      window.clearTimeout(timer);
    };
  }, [text, applied, search]);
  return (
    <TextField
      aria-label="Search all columns"
      placeholder="Search"
      controlSize="compact"
      className={extras.search}
      leadingIcon={<Icon name="search" />}
      value={text}
      title="Search every column"
      onChange={(event) => {
        setText(event.target.value);
      }}
      onKeyDown={(event) => {
        if (event.nativeEvent.isComposing) return;
        if (event.key === 'Enter') search(text);
        else if (event.key === 'Escape' && text) {
          event.stopPropagation();
          setText('');
          search('');
        }
      }}
    />
  );
}
