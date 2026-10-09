// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense } from 'react';
import type { EditorDialect } from '@openrows/editor';
import type { EditorHandle } from './request';

const SnippetPicker = lazy(() =>
  import('../snippets/SnippetPicker').then((module) => ({
    default: module.SnippetPicker,
  })),
);

/** Insert Snippet…: the picker (loaded on first use) inserting into the editor. */
export function TabSnippets(props: {
  dialect: EditorDialect;
  editorRef: { current: EditorHandle | null };
  onClose: () => void;
}): React.JSX.Element {
  const { editorRef } = props;
  return (
    <Suspense fallback={null}>
      <SnippetPicker
        dialect={props.dialect}
        onInsert={(body) => {
          editorRef.current?.insertSnippet(body);
        }}
        onClose={() => {
          props.onClose();
          editorRef.current?.focus();
        }}
      />
    </Suspense>
  );
}
