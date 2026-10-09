// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, SelectField, TextArea, TextField } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import { exportSnippets, importSnippets, snippetProblem } from './snippetStore';
import type { Snippet, SnippetScope } from './snippetStore';
import styles from './Snippets.module.css';

/** Engine choices of a snippet, labelled for people. */
export const SCOPE_OPTIONS: readonly { value: SnippetScope; label: string }[] =
  [
    { value: 'all', label: 'All engines' },
    { value: 'postgres', label: 'PostgreSQL' },
    { value: 'mysql', label: 'MySQL and MariaDB' },
    { value: 'sqlite', label: 'SQLite' },
  ];

/** Add or edit one user snippet; nothing changes until Save. */
export function SnippetEditor(props: {
  snippet: Snippet;
  isNew: boolean;
  onSave: (snippet: Snippet) => void;
  onClose: () => void;
}): React.JSX.Element {
  const [draft, setDraft] = useState(props.snippet);
  const [tried, setTried] = useState(false);
  const problem = snippetProblem(draft);
  const save = (): void => {
    setTried(true);
    if (!problem) props.onSave({ ...draft, name: draft.name.trim() });
  };
  return (
    <Sheet
      title={props.isNew ? 'Add snippet' : 'Edit snippet'}
      onClose={props.onClose}
    >
      <div className={styles.form}>
        <TextField
          label="Name"
          maxLength={80}
          data-initial-focus
          value={draft.name}
          onChange={(event) => {
            setDraft({ ...draft, name: event.target.value });
          }}
        />
        <TextField
          label="Prefix"
          maxLength={32}
          spellCheck={false}
          value={draft.prefix}
          onChange={(event) => {
            setDraft({ ...draft, prefix: event.target.value });
          }}
        />
        <SelectField
          label="Engines"
          value={draft.scope}
          options={SCOPE_OPTIONS}
          onValueChange={(scope) => {
            setDraft({ ...draft, scope });
          }}
        />
        <label className={styles.bodyLabel}>
          Body
          <TextArea
            aria-label="Body"
            className={styles.body}
            spellCheck={false}
            maxLength={10_000}
            value={draft.body}
            onChange={(event) => {
              setDraft({ ...draft, body: event.target.value });
            }}
          />
        </label>
        <p className={styles.help}>
          Typing the prefix offers the snippet in completion. Tab stops are
          written <code>{'${1:name}'}</code>; Tab moves to the next one.
        </p>
        {tried && problem && <p role="alert">{problem}</p>}
      </div>
      <SheetActions>
        <Button onClick={props.onClose}>Cancel</Button>
        <Button variant="primary" onClick={save}>
          Save
        </Button>
      </SheetActions>
    </Sheet>
  );
}

const importMessage = (error: unknown): string =>
  error instanceof SyntaxError
    ? 'Invalid JSON. Paste snippets exported from OpenRows.'
    : error instanceof Error
      ? error.message
      : 'Could not import snippets.';

/**
 * Import (paste JSON) or export (copy JSON) user snippets, like themes.
 * Imported snippets are added to the existing ones.
 */
export function SnippetTransfer(props: {
  /** The snippets to export; omitted to import. */
  items?: readonly Snippet[];
  onImport: (snippets: Snippet[]) => void;
  onClose: () => void;
}): React.JSX.Element {
  const importing = !props.items;
  const [text, setText] = useState(() =>
    props.items ? exportSnippets(props.items) : '',
  );
  const [message, setMessage] = useState('');
  const submit = (): void => {
    try {
      props.onImport(importSnippets(text));
    } catch (error) {
      setMessage(importMessage(error));
    }
  };
  const copy = async (): Promise<void> => {
    try {
      await navigator.clipboard.writeText(text);
      setMessage('Snippets JSON copied.');
    } catch {
      setMessage('Select the JSON below and copy it manually.');
    }
  };
  return (
    <Sheet
      title={importing ? 'Import snippets' : 'Export snippets'}
      onClose={props.onClose}
    >
      <p>
        {importing
          ? 'Paste a JSON array of snippets with name, prefix, body and scope.'
          : 'Copy this JSON to share your snippets.'}
      </p>
      <label className={styles.bodyLabel}>
        Snippets JSON
        <TextArea
          aria-label="Snippets JSON"
          className={styles.body}
          spellCheck={false}
          readOnly={!importing}
          value={text}
          onChange={(event) => {
            setText(event.target.value);
            setMessage('');
          }}
        />
      </label>
      {message && <p role={importing ? 'alert' : 'status'}>{message}</p>}
      <SheetActions>
        <Button onClick={props.onClose}>{importing ? 'Cancel' : 'Done'}</Button>
        {importing ? (
          <Button variant="primary" disabled={!text.trim()} onClick={submit}>
            Import
          </Button>
        ) : (
          <Button
            variant="primary"
            onClick={() => {
              void copy();
            }}
          >
            Copy JSON
          </Button>
        )}
      </SheetActions>
    </Sheet>
  );
}
