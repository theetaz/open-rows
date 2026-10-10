// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import { onNativeFilesReady, takeNativeFiles } from '@openrows/ipc';
import type { NativeFileDto } from '@openrows/ipc';
import type { Connections } from '../connections';
import type { QueryTabs, TabScope } from '../query';

/** Files without a selected database stay editable in a disconnected local scope. */
export const localFileScope: TabScope = {
  connectionId: 'local-files',
  database: 'Files',
};

/** Route validated handoffs to a new editor or a read-only-by-default connection form. */
export function acceptNativeFile(
  file: NativeFileDto,
  input: {
    tabs: QueryTabs;
    scope?: TabScope | undefined;
    connections: Connections;
  },
): string | null {
  if (file.kind === 'rejected') return file.message;
  if (file.kind === 'database') {
    if (file.engine !== 'sqlite' && file.engine !== 'duckdb')
      return 'Unsupported local database engine';
    input.connections.setSheet({
      kind: 'new-file',
      path: file.path,
      engine: file.engine,
    });
    return null;
  }
  const { opened } = file;
  input.tabs.open(input.scope ?? localFileScope, {
    title: opened.file.name,
    sql: opened.sql,
    file: {
      path: opened.file.path,
      name: opened.file.name,
      saved: opened.sql,
      ...(opened.file.modifiedMs === null
        ? {}
        : { modifiedMs: opened.file.modifiedMs }),
    },
  });
  return null;
}

/** Register before initial drain so launch-time and later OS events cannot race. */
export function useNativeFiles(input: {
  tabs: QueryTabs;
  scope?: TabScope | undefined;
  connections: Connections;
  enabled: boolean;
}): string | null {
  const latest = useRef(input);
  useEffect(() => {
    latest.current = input;
  }, [input]);
  const [message, setMessage] = useState<string | null>(null);
  useEffect(() => {
    if (!input.enabled) return;
    let alive = true;
    let stop: (() => void) | undefined;
    const drain = async (): Promise<void> => {
      try {
        const files = await takeNativeFiles();
        if (alive)
          for (const file of files)
            setMessage(acceptNativeFile(file, latest.current));
      } catch (error) {
        if (alive)
          setMessage(error instanceof Error ? error.message : String(error));
      }
    };
    onNativeFilesReady(() => {
      void drain();
    }).then(
      (unlisten) => {
        if (!alive) {
          unlisten();
          return;
        }
        stop = unlisten;
        void drain();
      },
      (error: unknown) => {
        setMessage(String(error));
      },
    );
    const failure = (event: Event): void => {
      setMessage((event as CustomEvent<string>).detail);
    };
    window.addEventListener('openrows:file-error', failure);
    return () => {
      alive = false;
      stop?.();
      window.removeEventListener('openrows:file-error', failure);
    };
  }, [input.enabled]);
  return message;
}
