// SPDX-License-Identifier: Apache-2.0
import { useCallback, useState } from 'react';
import { Button, HighlightText, Icon, TreeItem } from '@openrows/ui';
import type { IconName, MenuEntry } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import { confirmDestructive } from '@openrows/ipc';
import type { SavedQueryDto } from '@openrows/ipc';
import { insertSavedSql } from './historyInsert';
import { ContextMenu } from '../../shell/ContextMenu';
import { substringIndices } from '../schema';
import { NameSheet } from './NameSheet';
import { deleteQuery, renameQuery, useSavedQueries } from './savedQueries';
import { t } from './initialMessages';

type Props = {
  /** The active connection; none lists nothing. */
  profileId: string | undefined;
  search: string;
  /** Open the query in a new tab. */
  onOpen: (query: SavedQueryDto) => void;
  /** Move the query into the open project; absent without a project. */
  onMoveToProject?: ((query: SavedQueryDto) => Promise<void>) | undefined;
};

type Dialog =
  | {
      kind: 'menu';
      query: SavedQueryDto;
      x: number;
      y: number;
      target: Element | null;
    }
  | { kind: 'rename' | 'delete'; query: SavedQueryDto };

const message = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

function deletePrompt(query: SavedQueryDto): {
  title: string;
  message: string;
  action: string;
} {
  return {
    title: t('deleteName', { name: query.name }),
    message: t('savedQueryRemoved'),
    action: t('delete'),
  };
}

/**
 * The sidebar's local queries: saved queries of the connection in the local
 * store; right-click to move one into the project, rename or delete.
 */
export function SavedQueryList(props: Props): React.JSX.Element {
  const { queries, loading, error } = useSavedQueries(props.profileId);
  const [dialog, setDialog] = useState<Dialog | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const close = useCallback(() => {
    setDialog(null);
  }, []);
  const remove = (query: SavedQueryDto): void => {
    setDialog(null);
    deleteQuery(query.id).catch((reason: unknown) => {
      setFailure(message(reason));
    });
  };
  const askDelete = (query: SavedQueryDto): void => {
    const fallback = (): void => {
      setDialog({ kind: 'delete', query });
    };
    confirmDestructive(deletePrompt(query)).then((answer) => {
      if (answer === null) fallback();
      else if (answer) remove(query);
    }, fallback);
  };
  const search = props.search.toLowerCase();
  const shown = queries.filter((query) =>
    query.name.toLowerCase().includes(search),
  );
  return (
    <>
      {shown.map((query) => (
        <SavedRow
          key={query.id}
          query={query}
          search={props.search}
          onOpen={props.onOpen}
          onMenu={setDialog}
        />
      ))}
      <ListNote
        loading={loading}
        error={failure ?? error}
        empty={!shown.length}
        filtered={Boolean(props.search)}
      />
      {dialog && (
        <QueryDialog
          dialog={dialog}
          onOpen={props.onOpen}
          onError={setFailure}
          onClose={close}
          onMove={mover(props.onMoveToProject, setFailure)}
          onRename={(query) => {
            setDialog({ kind: 'rename', query });
          }}
          onDelete={askDelete}
          onConfirmDelete={remove}
        />
      )}
    </>
  );
}

/** One saved query: click opens it, right-click opens its menu. */
function SavedRow(props: {
  query: SavedQueryDto;
  search: string;
  onOpen: (query: SavedQueryDto) => void;
  onMenu: (dialog: Dialog) => void;
}): React.JSX.Element {
  const { query } = props;
  return (
    <div
      onContextMenu={(event) => {
        event.preventDefault();
        props.onMenu({
          kind: 'menu',
          query,
          x: event.clientX,
          y: event.clientY,
          target: event.currentTarget.firstElementChild,
        });
      }}
    >
      <TreeItem
        label={query.name}
        glyph="≡"
        title={query.sql}
        selected={false}
        onSelect={() => {
          props.onOpen(query);
        }}
      >
        <HighlightText
          text={query.name}
          indices={substringIndices(query.name, props.search)}
        />
      </TreeItem>
    </div>
  );
}

/** Move to Project with its failure shown in the list; none without a project. */
function mover(
  move: Props['onMoveToProject'],
  fail: (message: string) => void,
): ((query: SavedQueryDto) => void) | undefined {
  if (!move) return undefined;
  return (query) => {
    move(query).catch((reason: unknown) => {
      fail(message(reason));
    });
  };
}

function ListNote(props: {
  loading: boolean;
  error: string | null;
  empty: boolean;
  filtered: boolean;
}): React.JSX.Element | null {
  if (props.error) return <p role="alert">{props.error}</p>;
  if (props.loading || !props.empty) return null;
  return (
    <p role="status">{props.filtered ? t('noMatches') : t('noLocalQueries')}</p>
  );
}

/** The row menu, the rename sheet or the in-app delete alert. */
function QueryDialog(props: {
  dialog: Dialog;
  onClose: () => void;
  onOpen: (query: SavedQueryDto) => void;
  onError: (error: string) => void;
  onRename: (query: SavedQueryDto) => void;
  onDelete: (query: SavedQueryDto) => void;
  onConfirmDelete: (query: SavedQueryDto) => void;
  onMove?: ((query: SavedQueryDto) => void) | undefined;
}): React.JSX.Element {
  const { dialog } = props;
  const { query } = dialog;
  if (dialog.kind === 'menu')
    return <SavedQueryMenu {...props} dialog={dialog} />;
  if (dialog.kind === 'rename')
    return (
      <NameSheet
        title={t('renameQuery')}
        label={t('name')}
        initial={query.name}
        action={t('rename')}
        onSubmit={(name) => renameQuery(query.id, name)}
        onClose={props.onClose}
      />
    );
  const prompt = deletePrompt(query);
  return (
    <Sheet title={prompt.title} size="alert" onClose={props.onClose}>
      <p>{prompt.message}</p>
      <SheetActions>
        <Button onClick={props.onClose}>{t('cancel')}</Button>
        <Button
          variant="destructive"
          onClick={() => {
            props.onConfirmDelete(query);
          }}
        >
          {prompt.action}
        </Button>
      </SheetActions>
    </Sheet>
  );
}

type MenuProps = Parameters<typeof QueryDialog>[0] & {
  dialog: Extract<Dialog, { kind: 'menu' }>;
};

const item = (
  label: string,
  icon: IconName,
  run: () => void,
  danger = false,
): MenuEntry => ({
  kind: 'item',
  label,
  icon: <Icon name={icon} />,
  run,
  danger,
});

/** Open, Insert SQL and Copy SQL. */
function runEntries(props: MenuProps, query: SavedQueryDto): MenuEntry[] {
  return [
    item(t('open'), 'terminal', () => {
      props.onOpen(query);
    }),
    item(t('insertSql'), 'arrowRight', () => {
      if (!insertSavedSql(query.sql)) props.onError(t('openQueryEditorBefore'));
    }),
    item(t('copySql'), 'copy', () => {
      navigator.clipboard.writeText(query.sql).catch((error: unknown) => {
        props.onError(message(error));
      });
    }),
  ];
}

/** Move to Project and Rename, then the destructive Delete. */
function editEntries(props: MenuProps, query: SavedQueryDto): MenuEntry[] {
  const { onMove } = props;
  return [
    ...(onMove
      ? [
          item(t('moveProject'), 'folder', () => {
            onMove(query);
          }),
        ]
      : []),
    item(t('rename2'), 'pencil', () => {
      props.onRename(query);
    }),
    { kind: 'separator' },
    item(
      t('delete2'),
      'trash',
      () => {
        props.onDelete(query);
      },
      true,
    ),
  ];
}

function SavedQueryMenu(props: MenuProps): React.JSX.Element {
  const { dialog } = props;
  const { query } = dialog;
  return (
    <ContextMenu
      at={{ x: dialog.x, y: dialog.y }}
      target={dialog.target}
      label={t('nameActions', { name: query.name })}
      onClose={props.onClose}
      entries={[
        ...runEntries(props, query),
        { kind: 'separator' },
        ...editEntries(props, query),
      ]}
    />
  );
}
