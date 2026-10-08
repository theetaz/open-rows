// SPDX-License-Identifier: Apache-2.0
import { syntaxTree } from '@codemirror/language';
import { RangeSetBuilder } from '@codemirror/state';
import type { Extension } from '@codemirror/state';
import { Decoration, EditorView, ViewPlugin } from '@codemirror/view';
import type { DecorationSet, ViewUpdate } from '@codemirror/view';
import { currentStatement } from './statement';

const line = Decoration.line({ class: 'cm-currentQuery' });

/** Line marks for the visible lines of the statement Run Current would run. */
function decorate(view: EditorView): DecorationSet {
  const { state } = view;
  const range = currentStatement(state, state.selection.main.head);
  if (!range) return Decoration.none;
  const builder = new RangeSetBuilder<Decoration>();
  let next = 0;
  for (const visible of view.visibleRanges) {
    const to = Math.min(visible.to, range.to);
    let pos = Math.max(visible.from, range.from, next);
    while (pos <= to) {
      const current = state.doc.lineAt(pos);
      builder.add(current.from, current.from, line);
      pos = current.to + 1;
      next = pos;
    }
  }
  return builder.finish();
}

function changed(update: ViewUpdate): boolean {
  return (
    update.docChanged ||
    update.selectionSet ||
    update.viewportChanged ||
    syntaxTree(update.startState) !== syntaxTree(update.state)
  );
}

/** Tint every line of the current statement (TablePlus "Highlight current query"). */
export function currentQueryHighlight(): Extension {
  return ViewPlugin.fromClass(
    class {
      decorations: DecorationSet;
      constructor(view: EditorView) {
        this.decorations = decorate(view);
      }
      update(update: ViewUpdate): void {
        if (changed(update)) this.decorations = decorate(update.view);
      }
    },
    { decorations: (plugin) => plugin.decorations },
  );
}
