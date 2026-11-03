// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, SelectField, TextArea, TextField } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import { exportSnippets, importSnippets, snippetProblem } from './snippetStore';
import type { Snippet, SnippetScope } from './snippetStore';
import styles from './Snippets.module.css';
import { rich, t } from './lazyMessages';

/** Engine choices of a snippet, labelled for people. */
export const SCOPE_OPTIONS: readonly { value: SnippetScope; label: string }[] =
  [
    { value: 'all', label: t('allEngines') },
    { value: 'postgres', label: t('postgresql') },
    { value: 'mysql', label: t('mysqlMariadb') },
    { value: 'sqlserver', label: t('sqlServer') },
    { value: 'sqlite', label: t('sqlite') },
    { value: 'clickhouse', label: t('clickhouse') },
    { value: 'mongodb', label: t('mongodb') },
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
      title={props.isNew ? t('addSnippet') : t('editSnippet')}
      onClose={props.onClose}
    >
      <div className={styles.form}>
        <TextField
          label={t('name')}
          maxLength={80}
          data-initial-focus
          value={draft.name}
          onChange={(event) => {
            setDraft({ ...draft, name: event.target.value });
          }}
        />
        <TextField
          label={t('prefix')}
          maxLength={32}
          spellCheck={false}
          value={draft.prefix}
          onChange={(event) => {
            setDraft({ ...draft, prefix: event.target.value });
          }}
        />
        <SelectField
          label={t('engines')}
          value={draft.scope}
          options={SCOPE_OPTIONS}
          onValueChange={(scope) => {
            setDraft({ ...draft, scope });
          }}
        />
        <label className={styles.bodyLabel}>
          {t('body')}
          <TextArea
            aria-label={t('body')}
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
          {rich('typingPrefixOffers', { value: <code>{'${1:name}'}</code> })}
        </p>
        {tried && problem && <p role="alert">{problem}</p>}
      </div>
      <SheetActions>
        <Button onClick={props.onClose}>{t('cancel')}</Button>
        <Button variant="primary" onClick={save}>
          {t('save')}
        </Button>
      </SheetActions>
    </Sheet>
  );
}

const importMessage = (error: unknown): string =>
  error instanceof SyntaxError
    ? t('invalidJsonPaste')
    : error instanceof Error
      ? error.message
      : t('couldNotImport');

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
      setMessage(t('snippetsJsonCopied'));
    } catch {
      setMessage(t('selectJsonBelow'));
    }
  };
  return (
    <Sheet
      title={importing ? t('importSnippets') : t('exportSnippets')}
      onClose={props.onClose}
    >
      <p>{importing ? t('pasteJsonArray') : t('copyJsonShare')}</p>
      <label className={styles.bodyLabel}>
        {t('snippetsJson')}
        <TextArea
          aria-label={t('snippetsJson')}
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
        <Button onClick={props.onClose}>
          {importing ? t('cancel') : t('done')}
        </Button>
        {importing ? (
          <Button variant="primary" disabled={!text.trim()} onClick={submit}>
            {t('import')}
          </Button>
        ) : (
          <Button
            variant="primary"
            onClick={() => {
              void copy();
            }}
          >
            {t('copyJson')}
          </Button>
        )}
      </SheetActions>
    </Sheet>
  );
}
