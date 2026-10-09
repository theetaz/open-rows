// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, Switch } from '@openrows/ui';
import { SettingRow } from '../appearance/InterfaceSettings';
import { useSearching } from '../settings/settingsSearch';
import { BUILTIN_SNIPPETS } from './builtins';
import { SCOPE_OPTIONS, SnippetEditor, SnippetTransfer } from './SnippetSheets';
import { snippetSettings, updateSnippets } from './snippetStore';
import type { Snippet, SnippetScope } from './snippetStore';
import styles from '../appearance/Appearance.module.css';

type Dialog =
  | { kind: 'edit'; snippet: Snippet; isNew: boolean }
  | { kind: 'import' }
  | { kind: 'export' };

const scopeLabel = (scope: SnippetScope): string =>
  SCOPE_OPTIONS.find((option) => option.value === scope)?.label ?? scope;

const blank = (): Snippet => ({
  id: `user-${crypto.randomUUID()}`,
  name: '',
  prefix: '',
  body: '',
  scope: 'all',
});

/** Replace the snippet with the same id, or add it. */
function saved(items: readonly Snippet[], snippet: Snippet): Snippet[] {
  const known = items.some((item) => item.id === snippet.id);
  return known
    ? items.map((item) => (item.id === snippet.id ? snippet : item))
    : [...items, snippet];
}

/**
 * Settings ▸ Snippets: the user's snippets (add, edit, delete, import and
 * export as JSON) and the built-in snippets, each of which can be turned off.
 */
export function SnippetsPane(): React.JSX.Element {
  const settings = snippetSettings.use();
  const [dialog, setDialog] = useState<Dialog | null>(null);
  const close = (): void => {
    setDialog(null);
  };
  return (
    <>
      <UserSnippets items={settings.items} onDialog={setDialog} />
      <BuiltinSnippets disabled={settings.disabledBuiltins} />
      {dialog?.kind === 'edit' && (
        <SnippetEditor
          snippet={dialog.snippet}
          isNew={dialog.isNew}
          onSave={(snippet) => {
            updateSnippets({ items: saved(settings.items, snippet) });
            close();
          }}
          onClose={close}
        />
      )}
      {(dialog?.kind === 'import' || dialog?.kind === 'export') && (
        <SnippetTransfer
          {...(dialog.kind === 'export' ? { items: settings.items } : {})}
          onImport={(snippets) => {
            updateSnippets({ items: [...settings.items, ...snippets] });
            close();
          }}
          onClose={close}
        />
      )}
    </>
  );
}

function UserSnippets(props: {
  items: readonly Snippet[];
  onDialog: (dialog: Dialog) => void;
}): React.JSX.Element {
  const searching = useSearching();
  const { items, onDialog } = props;
  return (
    <section aria-label="Your snippets">
      <div className={styles.sectionHeader}>
        <div>
          <h3>Your snippets</h3>
          <p>Type a prefix in the editor, or use Insert Snippet….</p>
        </div>
        {!searching && (
          <div className={styles.actions}>
            <Button
              size="sm"
              onClick={() => {
                onDialog({ kind: 'edit', snippet: blank(), isNew: true });
              }}
            >
              Add snippet
            </Button>
            <Button
              size="sm"
              onClick={() => {
                onDialog({ kind: 'import' });
              }}
            >
              Import…
            </Button>
            <Button
              size="sm"
              disabled={!items.length}
              onClick={() => {
                onDialog({ kind: 'export' });
              }}
            >
              Export…
            </Button>
          </div>
        )}
      </div>
      {!items.length && !searching && <p>No snippets of your own yet.</p>}
      <div className={styles.settingsGroup}>
        {items.map((snippet) => (
          <UserSnippetRow key={snippet.id} snippet={snippet} {...props} />
        ))}
      </div>
    </section>
  );
}

function UserSnippetRow(props: {
  snippet: Snippet;
  items: readonly Snippet[];
  onDialog: (dialog: Dialog) => void;
}): React.JSX.Element {
  const { snippet } = props;
  return (
    <SettingRow
      title={`${snippet.prefix} — ${snippet.name}`}
      description={scopeLabel(snippet.scope)}
    >
      <Button
        size="sm"
        aria-label={`Edit ${snippet.name}`}
        onClick={() => {
          props.onDialog({ kind: 'edit', snippet, isNew: false });
        }}
      >
        Edit
      </Button>
      <Button
        size="sm"
        variant="destructive"
        aria-label={`Delete ${snippet.name}`}
        onClick={() => {
          updateSnippets({
            items: props.items.filter((item) => item.id !== snippet.id),
          });
        }}
      >
        Delete
      </Button>
    </SettingRow>
  );
}

function BuiltinSnippets(props: {
  disabled: readonly string[];
}): React.JSX.Element {
  const toggle = (id: string, enabled: boolean): void => {
    const others = props.disabled.filter((item) => item !== id);
    updateSnippets({ disabledBuiltins: enabled ? others : [...others, id] });
  };
  return (
    <section aria-label="Built-in snippets">
      <h3>Built-in snippets</h3>
      <div className={styles.settingsGroup}>
        {BUILTIN_SNIPPETS.map((snippet) => (
          <SettingRow
            key={snippet.id}
            title={`${snippet.prefix} — ${snippet.name}`}
            description={scopeLabel(snippet.scope)}
          >
            <Switch
              aria-label={`${snippet.name} (${scopeLabel(snippet.scope)})`}
              checked={!props.disabled.includes(snippet.id)}
              onCheckedChange={(checked) => {
                toggle(snippet.id, checked);
              }}
            />
          </SettingRow>
        ))}
      </div>
    </section>
  );
}
