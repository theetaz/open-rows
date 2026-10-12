// SPDX-License-Identifier: Apache-2.0
import { useCallback, useEffect, useState } from 'react';
import {
  approveBoard,
  deleteBoard,
  importBoard,
  listBoards,
  saveBoard,
} from '@openrows/ipc';
import type { Board, BoardDto } from '@openrows/ipc';

export type Boards = {
  boards: BoardDto[];
  selected: BoardDto | null;
  error: string | null;
  select: (id: string) => void;
  save: (board: Board, id: string | null) => Promise<void>;
  importText: (text: string) => Promise<void>;
  approve: (id: string) => Promise<void>;
  remove: (id: string) => Promise<void>;
};

function message(error: unknown): string {
  if (error && typeof error === 'object' && 'message' in error)
    return String(error.message);
  return String(error);
}

/** Locally stored boards of one profile; nothing here runs a query. */
export function useBoards(profileId: string, enabled: boolean): Boards {
  const [boards, setBoards] = useState<BoardDto[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const reload = useCallback(
    async (select?: string): Promise<void> => {
      const list = await listBoards(profileId);
      setBoards(list);
      setSelectedId((current) => select ?? current ?? list[0]?.id ?? null);
    },
    [profileId],
  );
  useEffect(() => {
    if (!enabled) return;
    let alive = true;
    listBoards(profileId).then(
      (list) => {
        if (!alive) return;
        setBoards(list);
        setSelectedId((current) => current ?? list[0]?.id ?? null);
      },
      (cause: unknown) => {
        if (alive) setError(message(cause));
      },
    );
    return () => {
      alive = false;
    };
  }, [enabled, profileId]);
  const attempt = useCallback(
    async (action: () => Promise<string | undefined>): Promise<void> => {
      try {
        const select = await action();
        setError(null);
        await reload(select);
      } catch (cause) {
        setError(message(cause));
      }
    },
    [reload],
  );
  const selected =
    boards.find((board) => board.id === selectedId) ?? boards[0] ?? null;
  return {
    boards,
    selected,
    error,
    select: setSelectedId,
    save: (board, id) =>
      attempt(async () => (await saveBoard(profileId, id, board)).id),
    importText: (text) =>
      attempt(async () => (await importBoard(profileId, text)).id),
    approve: (id) =>
      attempt(async () => {
        await approveBoard(profileId, id);
        return id;
      }),
    remove: (id) =>
      attempt(async () => {
        await deleteBoard(profileId, id);
        setSelectedId(null);
        return undefined;
      }),
  };
}
