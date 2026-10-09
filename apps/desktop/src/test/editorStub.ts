// SPDX-License-Identifier: Apache-2.0
import type { SqlEditorOptions } from '@openrows/editor';

/**
 * `@openrows/editor` for jsdom: a plain text area standing in for the SQL
 * editor, reporting edits through `onDocChange` like CodeMirror does.
 */
export function editorStub(): Record<string, unknown> {
  return {
    mountSqlEditor: (parent: HTMLElement, options: SqlEditorOptions) => {
      const area = document.createElement('textarea');
      area.setAttribute('aria-label', options.label);
      area.value = options.doc;
      let doc = options.doc;
      area.addEventListener('input', () => {
        doc = area.value;
        options.onDocChange?.();
      });
      parent.appendChild(area);
      return {
        view: { state: { doc: { toString: () => doc } } },
        destroy: () => {
          area.remove();
        },
      };
    },
  };
}
