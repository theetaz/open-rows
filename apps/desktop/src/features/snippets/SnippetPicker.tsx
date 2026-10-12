// SPDX-License-Identifier: Apache-2.0
import { Palette } from '@openrows/ui/palette';
import type { EditorDialect } from '@openrows/editor';
import { projectSnippet, useProjectSql } from '../projects/projectSql';
import { snippetSettings, snippetsFor } from './snippetStore';

/**
 * Insert Snippet…: the engine's snippets in a searchable palette; the
 * chosen one is inserted at the cursor with its first field selected.
 */
export function SnippetPicker(props: {
  dialect: EditorDialect;
  onInsert: (body: string) => void;
  onClose: () => void;
}): React.JSX.Element {
  const shared = useProjectSql()
    .filter((item) => item.snippet)
    .flatMap((item) => {
      const snippet = projectSnippet(item);
      const scope = props.dialect === 'mariadb' ? 'mysql' : props.dialect;
      return snippet && (snippet.scope === 'all' || snippet.scope === scope)
        ? [snippet]
        : [];
    });
  const snippets = [
    ...snippetsFor(snippetSettings.useValue(), props.dialect),
    ...shared,
  ];
  const items = snippets.map((snippet) => ({
    id: snippet.id,
    label: snippet.name,
    detail: snippet.prefix,
    searchText: `${snippet.name} ${snippet.prefix}`,
  }));
  return (
    <Palette
      title="Insert snippet"
      items={items}
      onClose={props.onClose}
      onChoose={(id) => {
        props.onClose();
        const chosen = snippets.find((snippet) => snippet.id === id);
        if (chosen) props.onInsert(chosen.body);
      }}
    />
  );
}
