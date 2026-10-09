// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import {
  attachDatabase,
  detachDatabase,
  pickConnectionFile,
} from '@openrows/ipc';
import type { AttachReply, ProfileDto } from '@openrows/ipc';
import type { Connections } from '../connections';
import { aliasFor, attachedAliases, attachedSchema } from './attachAlias';
import type { Catalog } from './useCatalog';

/** The file chosen in the open panel, waiting for its schema name. */
export type PendingAttach = {
  path: string;
  alias: string;
  error: string | null;
  busy: boolean;
};

type AttachTools = {
  /** The active connection is a connected `SQLite` database. */
  available: boolean;
  /** The selected schema when it is an attached database. */
  detachable: string | undefined;
  pending: PendingAttach | null;
  /** Detach failures and sessions that could not follow a change. */
  notice: string | null;
  choose: () => void;
  setAlias: (alias: string) => void;
  confirm: () => void;
  cancel: () => void;
  detach: () => void;
};

const message = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

/** The connection is saved again and the schema list re-read after a change. */
function applied(
  reply: AttachReply,
  connections: Connections,
  catalog: Catalog,
): string | null {
  connections.saved(reply.profile);
  catalog.refresh();
  return reply.warnings.length ? reply.warnings.join(' ') : null;
}

/**
 * "Attach Database…" for `SQLite`: the native open panel, then a schema
 * name (prefilled from the file name); "Detach" for the selected attached
 * schema. Changes are saved in the connection and applied to its open tabs.
 */
export function useAttachDatabase(
  connections: Connections,
  catalog: Catalog,
): AttachTools {
  const [pending, setPending] = useState<PendingAttach | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const profile: ProfileDto | undefined = connections.active;
  const available =
    profile?.engine === 'sqlite' && !catalog.offline && !!catalog.database;
  const confirm = (): void => {
    if (!profile || !pending || pending.busy) return;
    setPending({ ...pending, busy: true, error: null });
    attachDatabase(profile.id, pending.path, pending.alias.trim()).then(
      (reply) => {
        setPending(null);
        setNotice(applied(reply, connections, catalog));
        catalog.setSchema(pending.alias.trim());
      },
      (error: unknown) => {
        setPending({ ...pending, busy: false, error: message(error) });
      },
    );
  };
  return {
    available,
    detachable: available ? attachedSchema(profile, catalog.schema) : undefined,
    pending,
    notice,
    choose: () => {
      setNotice(null);
      pickConnectionFile('sqliteDatabase').then(
        (path) => {
          if (!path) return;
          const alias = aliasFor(path, attachedAliases(profile));
          setPending({ path, alias, error: null, busy: false });
        },
        (error: unknown) => {
          setNotice(message(error));
        },
      );
    },
    setAlias: (alias) => {
      if (pending) setPending({ ...pending, alias, error: null });
    },
    confirm,
    cancel: () => {
      setPending(null);
    },
    detach: () => {
      const alias = attachedSchema(profile, catalog.schema);
      if (!profile || !alias) return;
      setNotice(null);
      detachDatabase(profile.id, alias).then(
        (reply) => {
          setNotice(applied(reply, connections, catalog));
        },
        (error: unknown) => {
          setNotice(message(error));
        },
      );
    },
  };
}
