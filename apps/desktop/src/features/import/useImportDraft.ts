// SPDX-License-Identifier: Apache-2.0
import { useCallback, useEffect, useRef, useState } from 'react';
import {
  listObjects,
  pickImportFile,
  previewImport,
  tableEditInfo,
} from '@openrows/ipc';
import type {
  EditInfoDto,
  ImportPreviewDto,
  ImportPreviewRequest,
} from '@openrows/ipc';
import {
  defaultColumns,
  defaultMapping,
  initialDraft,
  previewTypes,
} from './model';
import type { ImportContext, ImportDraft } from './model';

/** Wait after the last change before previewing again. */
const PREVIEW_DELAY_MS = 150;

const messageOf = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

const sourceKey = (preview: ImportPreviewDto | null): string =>
  preview ? preview.columns.map((column) => column.name).join('\u0000') : '';

export type ImportDraftState = {
  draft: ImportDraft;
  update: (patch: Partial<ImportDraft>) => void;
  preview: ImportPreviewDto | null;
  /** The existing target table's columns, key and safety switches. */
  table: EditInfoDto | null;
  /** Tables of the schema, for the target picker. */
  tables: string[];
  error: string | null;
  loading: boolean;
  pick: () => void;
};

/** The sheet's choices, the file preview and the target table's shape. */
export function useImportDraft(context: ImportContext): ImportDraftState {
  const [draft, setDraft] = useState(() => initialDraft(context));
  const [preview, setPreview] = useState<ImportPreviewDto | null>(null);
  const [error, setError] = useState<string | null>(null);
  /** Request key of the last preview that arrived (or failed). */
  const [loaded, setLoaded] = useState('');
  const table = useTargetTable(context, draft, setError);
  const tables = useSchemaTables(context);
  const update = useCallback((patch: Partial<ImportDraft>): void => {
    setDraft((current) => ({ ...current, ...patch }));
  }, []);
  const types = previewTypes(
    draft,
    table?.columns ?? [],
    preview?.columns.length ?? 0,
  );
  const request: ImportPreviewRequest | null = draft.file
    ? {
        fileId: draft.file.fileId,
        profileId: context.profileId,
        options: draft.options,
        types,
      }
    : null;
  const key = request ? JSON.stringify(request) : '';
  useEffect(() => {
    if (!key) return;
    let live = true;
    const timer = setTimeout(() => {
      previewImport(JSON.parse(key) as ImportPreviewRequest).then(
        (next) => {
          if (!live) return;
          setPreview(next);
          setLoaded(key);
          setError(null);
          setDraft((current) => ({
            ...current,
            options: current.options ?? next.options,
          }));
        },
        (failure: unknown) => {
          if (!live) return;
          setLoaded(key);
          setError(messageOf(failure));
        },
      );
    }, PREVIEW_DELAY_MS);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [key]);
  useDefaults({ preview, table, setDraft });
  const pick = useCallback((): void => {
    pickImportFile().then(
      (reply) => {
        if (reply.status !== 'picked') return;
        setPreview(null);
        setDraft((current) => ({
          ...current,
          file: { fileId: reply.fileId, fileName: reply.fileName },
          options: null,
        }));
      },
      (failure: unknown) => {
        setError(messageOf(failure));
      },
    );
  }, []);
  const loading = key !== '' && key !== loaded;
  return { draft, update, preview, table, tables, error, loading, pick };
}

/**
 * Reset the mapping and new-table columns when the file's columns or the
 * target table change, but not on every preview refresh.
 */
function useDefaults(input: {
  preview: ImportPreviewDto | null;
  table: EditInfoDto | null;
  setDraft: React.Dispatch<React.SetStateAction<ImportDraft>>;
}): void {
  const { preview, table, setDraft } = input;
  const seen = useRef({ sources: '', table: null as EditInfoDto | null });
  useEffect(() => {
    const sources = sourceKey(preview);
    if (
      !preview ||
      (seen.current.sources === sources && seen.current.table === table)
    )
      return;
    const sourcesChanged = seen.current.sources !== sources;
    seen.current = { sources, table };
    setDraft((current) => ({
      ...current,
      columns: sourcesChanged
        ? defaultColumns(preview.columns)
        : current.columns,
      mapping: table ? defaultMapping(table.columns, preview.columns) : {},
      key: table ? table.key : [],
    }));
  }, [preview, table, setDraft]);
}

/** The existing target table's shape; `null` for a new table. */
function useTargetTable(
  context: ImportContext,
  draft: ImportDraft,
  setError: (error: string | null) => void,
): EditInfoDto | null {
  const [loaded, setLoaded] = useState<{
    name: string;
    info: EditInfoDto;
  } | null>(null);
  const name = draft.target === 'existing' ? draft.table.trim() : '';
  const { profileId, database, schema } = context;
  useEffect(() => {
    if (!name) return;
    let live = true;
    tableEditInfo({ profileId, database, schema, table: name }).then(
      (info) => {
        if (live) setLoaded({ name, info });
      },
      (failure: unknown) => {
        if (live) setError(messageOf(failure));
      },
    );
    return () => {
      live = false;
    };
  }, [profileId, database, schema, name, setError]);
  return loaded?.name === name ? loaded.info : null;
}

/** Table names of the schema, for the existing-table picker. */
function useSchemaTables(context: ImportContext): string[] {
  const [tables, setTables] = useState<string[]>([]);
  const { profileId, database, schema } = context;
  useEffect(() => {
    let live = true;
    listObjects(
      profileId,
      database,
      { schema, kinds: ['table'], limit: null },
      false,
    ).then(
      (reply) => {
        if (live) setTables(reply.data.map((object) => object.name));
      },
      // Without the list the picker falls back to typing the name.
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, [profileId, database, schema]);
  return tables;
}
