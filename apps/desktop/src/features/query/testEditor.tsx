// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef } from 'react';
import type { EditorHandle, RunScope } from './request';

/** jsdom stand-in for the CodeMirror editor with the same props and handle. */
export function TextareaEditor(props: {
  doc: string;
  label: string;
  onChange: (sql: string) => void;
  onRun: (scope: RunScope) => void;
  onReady: (editor: EditorHandle | null) => void;
}): React.JSX.Element {
  const area = useRef<HTMLTextAreaElement>(null);
  const { onReady } = props;
  useEffect(() => {
    const element = area.current;
    if (!element) return;
    onReady({
      text: () => element.value,
      cursor: () => element.selectionEnd,
      selection: () =>
        element.value.slice(element.selectionStart, element.selectionEnd),
      focus: () => {
        element.focus();
      },
    });
    return () => {
      onReady(null);
    };
  }, [onReady]);
  return (
    <textarea
      ref={area}
      aria-label={props.label}
      defaultValue={props.doc}
      onChange={(event) => {
        props.onChange(event.target.value);
      }}
    />
  );
}
