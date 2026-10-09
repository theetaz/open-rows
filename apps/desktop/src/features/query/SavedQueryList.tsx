// SPDX-License-Identifier: Apache-2.0
import { useCallback, useState } from 'react';
import { Button, HighlightText, TreeItem } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import { confirmDestructive } from '@openrows/ipc';
import type { SavedQueryDto } from '@openrows/ipc';
import { CascadeMenu } from '../../shell/CascadeMenu';
import { substringIndices } from '../schema';
import { NameSheet } from './NameSheet';
import { deleteQuery, renameQuery, useSavedQueries } from './savedQueries';

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
  | { kind: 'menu'; query: SavedQueryDto; x: number; y: number }
  | { kind: 'rename' | 'delete'; query: SavedQueryDto };

const message = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

function deletePrompt(query: SavedQueryDto): {
  title: string;
  message: string;
  action: string;
} {
  return {
    title: `Delete “${query.name}”?`,
    message: 'The saved query is removed from this connection.',
    action: 'Delete',
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
        <div
          key={query.id}
          onContextMenu={(event) => {
            event.preventDefault();
            setDialog({
              kind: 'menu',
              query,
              x: event.clientX,
              y: event.clientY,
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
    <p role="status">
      {props.filtered
        ? 'No matches'
        : 'No local queries. Use Run ▸ Add to Queries… in a query tab.'}
    </p>
  );
}

/** The row menu, the rename sheet or the in-app delete alert. */
function QueryDialog(props: {
  dialog: Dialog;
  onClose: () => void;
  onRename: (query: SavedQueryDto) => void;
  onDelete: (query: SavedQueryDto) => void;
  onConfirmDelete: (query: SavedQueryDto) => void;
  onMove?: ((query: SavedQueryDto) => void) | undefined;
}): React.JSX.Element {
  const { dialog, onMove } = props;
  const { query } = dialog;
  if (dialog.kind === 'menu')
    return (
      <CascadeMenu
        x={dialog.x}
        y={dialog.y}
        label={`${query.name} actions`}
        onClose={props.onClose}
        entries={[
          ...(onMove
            ? [
                {
                  kind: 'item' as const,
                  label: 'Move to Project',
                  run: () => {
                    onMove(query);
                  },
                },
              ]
            : []),
          {
            kind: 'item',
            label: 'Rename…',
            run: () => {
              props.onRename(query);
            },
          },
          {
            kind: 'item',
            label: 'Delete…',
            run: () => {
              props.onDelete(query);
            },
          },
        ]}
      />
    );
  if (dialog.kind === 'rename')
    return (
      <NameSheet
        title="Rename query"
        label="Name"
        initial={query.name}
        action="Rename"
        onSubmit={(name) => renameQuery(query.id, name)}
        onClose={props.onClose}
      />
    );
  const prompt = deletePrompt(query);
  return (
    <Sheet title={prompt.title} size="alert" onClose={props.onClose}>
      <p>{prompt.message}</p>
      <SheetActions>
        <Button onClick={props.onClose}>Cancel</Button>
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
