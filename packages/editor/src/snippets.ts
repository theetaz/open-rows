// SPDX-License-Identifier: Apache-2.0
import { snippet, snippetCompletion } from '@codemirror/autocomplete';
import type {
  Completion,
  CompletionContext,
  CompletionResult,
  CompletionSource,
} from '@codemirror/autocomplete';
import type { EditorView } from '@codemirror/view';
import { completionKinds } from './completion/kinds';

/**
 * A SQL snippet: typing `prefix` offers it in completion; `body` is a
 * CodeMirror template whose `${1:name}` fields are tab stops.
 */
export type SqlSnippet = {
  name: string;
  prefix: string;
  body: string;
  /** Saved queries insert literal SQL, preserving parameter and template text. */
  literal?: boolean;
};

/** The snippets in effect, read on every completion so edits apply live. */
export type SnippetSource = () => readonly SqlSnippet[];

const PREFIX = /[\w$]+/;

/** Shown in the completion info panel: the body without field markers. */
export function snippetPreview(body: string): string {
  return body
    .replace(/[$#]\{(?:\d+:?)?([^}]*)\}/g, '$1')
    .replace(/\\([{}])/g, '$1');
}

function toCompletion(item: SqlSnippet): Completion {
  if (item.literal)
    return {
      label: item.prefix,
      detail: item.name,
      type: 'snippet',
      info: item.body,
      apply: item.body,
      boost: 1,
    };
  return snippetCompletion(item.body, {
    label: item.prefix,
    detail: item.name,
    type: 'snippet',
    info: snippetPreview(item.body),
    boost: 1,
  });
}

/**
 * Completion of snippets whose prefix starts with the typed word; listed
 * alongside the SQL completion, and only while a word is being typed.
 */
export function snippetSource(source: SnippetSource): CompletionSource {
  return (context: CompletionContext): CompletionResult | null => {
    if (!context.state.facet(completionKinds).snippets) return null;
    const word = context.matchBefore(PREFIX);
    if (!word || word.from === word.to) return null;
    const typed = word.text.toLowerCase();
    const options = source()
      .filter((item) => item.prefix.toLowerCase().startsWith(typed))
      .map(toCompletion);
    if (!options.length) return null;
    return { from: word.from, options, validFor: PREFIX };
  };
}

/** Insert a snippet over the selection and select its first field. */
export function insertSnippet(view: EditorView, body: string): void {
  const { from, to } = view.state.selection.main;
  snippet(body)(view, null, from, to);
  view.focus();
}
