// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import type { EditorDialect, FormatKind } from '@openrows/editor';
import { formatOptionsOf } from './editorSettings';
import type { EditorSettings } from './editorSettings';
import type { EditorHandle } from './request';

const message = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

/** Beautify/uglify through the editor; a formatter error becomes a notice. */
export function useFormat(
  editor: { current: EditorHandle | null },
  settings: EditorSettings,
  dialect: EditorDialect,
): [string | null, (kind: FormatKind) => void] {
  const [failure, setFailure] = useState<string | null>(null);
  const format = (kind: FormatKind): void => {
    editor.current?.format(kind, formatOptionsOf(settings, dialect)).then(
      () => {
        setFailure(null);
      },
      (error: unknown) => {
        setFailure(`Could not format: ${message(error)}`);
      },
    );
  };
  return [failure, format];
}
