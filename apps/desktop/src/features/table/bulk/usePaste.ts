// SPDX-License-Identifier: Apache-2.0
import { useEffect } from 'react';
import type { SelectionModel } from '@openrows/grid';
import type { ResultMeta } from '../../query/run';
import type { InlineEditor } from '../inline/useInlineEditor';
import type { RowEditing } from '../useRowEditing';
import { dataRanges } from './cells';

/** What a paste needs from the grid at the moment it happens. */
type PasteContext = {
  editor: InlineEditor;
  editing: RowEditing;
  result: ResultMeta;
  rowNumbers: boolean;
};

/** ⌘V over the grid pastes into the selection: one value fills it, a TSV block lands from its top-left. */
export function usePaste(
  host: React.RefObject<HTMLElement | null>,
  latest: React.RefObject<PasteContext>,
  selectionRef: React.RefObject<SelectionModel>,
): void {
  useEffect(() => {
    const element = host.current;
    if (!element) return;
    const paste = (event: ClipboardEvent): void => {
      const { editor, editing, result, rowNumbers } = latest.current;
      const text = event.clipboardData?.getData('text/plain') ?? '';
      if (editor.open || !text) return;
      event.preventDefault();
      editing.bulk.paste(
        result,
        dataRanges(selectionRef.current.ranges, rowNumbers),
        text,
      );
    };
    element.addEventListener('paste', paste);
    return () => {
      element.removeEventListener('paste', paste);
    };
  }, [host, latest, selectionRef]);
}
