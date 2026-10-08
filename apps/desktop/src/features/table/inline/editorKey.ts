// SPDX-License-Identifier: Apache-2.0
import type { InlineEditor, OpenEditor } from './useInlineEditor';

type Props = { open: OpenEditor; editor: InlineEditor };

/** Enter stages, Escape cancels, Tab/arrows stage and move, ⌘⌫ stages NULL. */
export function editorKey(
  event: React.KeyboardEvent<HTMLInputElement | HTMLTextAreaElement>,
  { open, editor }: Props,
  multiline: boolean,
): void {
  const value = event.currentTarget.value;
  const commands: Record<string, (() => void) | undefined> = {
    Escape: editor.cancel,
    Backspace: event.metaKey
      ? () => {
          if (open.edit.nullable) editor.commit(null);
        }
      : undefined,
    Enter:
      (multiline ? event.metaKey : !event.shiftKey && !event.altKey) &&
      !event.nativeEvent.isComposing
        ? () => {
            editor.commit(value);
          }
        : undefined,
  };
  if (!multiline) {
    commands.Tab = () => {
      editor.commit(value, event.shiftKey ? 'previous' : 'next');
    };
    commands.ArrowUp = () => {
      editor.commit(value, 'up');
    };
    commands.ArrowDown = () => {
      editor.commit(value, 'down');
    };
  }
  const command = commands[event.key];
  if (!command) return;
  event.preventDefault();
  event.stopPropagation();
  command();
}
