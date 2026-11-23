// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import type { ObjectKindDto } from '@openrows/ipc';
import type { Connections } from '../connections';
import type { Catalog, ObjectRef } from '../schema';
import { OBJECT_PAGE } from '../schema/useCatalog';
import type { QueryTab, TabPlace } from '../query/tabs';
import type { PathLevel, PathSegment } from './pathModel';
import { NEW_CONNECTION, PICKER_GROUPS } from './pathPickers';

/** ⌘L asks the path bar to open its last segment. */
export const OPEN_LOCATION = 'openrows:open-location';

/** What choosing in a picker does to the workspace. */
export type PathActions = {
  connections: Connections;
  catalog: Catalog;
  tab: QueryTab | undefined;
  /** Open the connection flow. */
  onNewConnection: () => void;
  /** Open (or focus) an object's tab; `newTab` is ⌘↩. */
  onObject: (object: ObjectRef, newTab: boolean) => void;
  /** Show another table in the table tab (with history). */
  onNavigate: (id: string, place: TabPlace) => void;
};

/** The open picker: its level and the segment it hangs from. */
type OpenPicker = { level: PathLevel; anchor: Element };

/** The path bar's picker state and what its segments and keys do. */
type PathPicker = {
  open: OpenPicker | null;
  /** The schema the object picker lists: one just chosen, else the path's. */
  schema: string | null;
  toggle: (level: PathLevel) => void;
  /** The picker of `level` closed itself (another may have opened since). */
  close: (level: PathLevel) => void;
  /** ← → with an empty search: the neighbouring segment's picker. */
  step: (dir: -1 | 1) => void;
  pick: (
    level: PathLevel,
    id: string,
    newTab: boolean,
    current: string,
  ) => void;
};

const LEVELS: readonly PathLevel[] = [
  'connection',
  'database',
  'schema',
  'object',
];
/** The pause before a short path opens its next picker by itself. */
const CASCADE_MS = 120;
/** How long after a choice a path that turns short still cascades. */
const CASCADE_WINDOW_MS = 3000;

/** The segment button of `level` inside the bar. */
function segmentOf(bar: HTMLElement | null, level: PathLevel): Element | null {
  return bar?.querySelector(`[data-level="${level}"]`) ?? null;
}

/** A pick in the object picker, as the tree's object reference. */
function objectOf(id: string, database: string, schema: string): ObjectRef {
  const [kind = 'table', name = ''] = id.split('\n');
  return {
    database,
    schema,
    name,
    kind: kind as ObjectKindDto,
    arguments: null,
  };
}

/** Run a pick at `level`; returns whether the path now stops short on purpose. */
function choose(
  actions: PathActions,
  level: PathLevel,
  pick: { id: string; newTab: boolean; schema: string },
): boolean {
  const { connections, catalog, tab } = actions;
  if (level === 'connection') {
    if (pick.id === NEW_CONNECTION) actions.onNewConnection();
    else connections.select(pick.id);
    return pick.id !== NEW_CONNECTION;
  }
  if (level === 'schema') {
    catalog.setSchema(pick.id);
    return false;
  }
  if (level === 'database') {
    catalog.openDatabase(pick.id);
    return true;
  }
  const database = tab?.database || catalog.database;
  const object = objectOf(pick.id, database, pick.schema);
  if (tab?.kind === 'table' && !pick.newTab)
    actions.onNavigate(tab.id, {
      connectionId: tab.connectionId,
      database,
      table: {
        database,
        schema: pick.schema,
        name: object.name,
        ...(object.kind === 'table' ? {} : { kind: object.kind }),
      },
    });
  else actions.onObject(object, pick.newTab);
  return false;
}

/**
 * The path bar's pickers: which one is open, ⌘L, ← → between segments,
 * and the cascade that opens the next level when a choice leaves the path
 * short ("three levels are three keystrokes of Enter").
 */
export function usePathPicker(
  bar: React.RefObject<HTMLElement | null>,
  segments: readonly PathSegment[],
  actions: PathActions,
): PathPicker {
  const [open, setOpen] = useState<OpenPicker | null>(null);
  const [schema, setSchema] = useState<string | null>(null);
  const [wave, setWave] = useState(0);
  const show = (level: PathLevel, listed = schema): void => {
    const anchor = segmentOf(bar.current, level);
    if (!anchor) return;
    if (level === 'object')
      for (const group of PICKER_GROUPS)
        actions.catalog.loadObjects(
          group,
          OBJECT_PAGE,
          listed ?? actions.catalog.schema,
        );
    setOpen({ level, anchor });
  };
  const last = segments.at(-1);
  const short = last?.role === 'todo' ? last.level : null;
  useEffect(() => {
    // Only the path a choice just left short (the scope switch may take a moment).
    if (!short || !wave || Date.now() - wave > CASCADE_WINDOW_MS) return;
    const timer = window.setTimeout(() => {
      setWave(0);
      const anchor = segmentOf(bar.current, short);
      if (anchor) setOpen({ level: short, anchor });
    }, CASCADE_MS);
    return () => {
      window.clearTimeout(timer);
    };
  }, [short, wave, bar]);
  const latest = useRef({ show, last });
  useEffect(() => {
    latest.current = { show, last };
  });
  useEffect(() => {
    const location = (): void => {
      const { show: open, last: end } = latest.current;
      if (end) open(end.level);
    };
    window.addEventListener(OPEN_LOCATION, location);
    return () => {
      window.removeEventListener(OPEN_LOCATION, location);
    };
  }, []);
  return {
    open,
    schema,
    toggle: (level: PathLevel) => {
      if (open?.level === level) setOpen(null);
      else show(level);
    },
    close: (level: PathLevel) => {
      setOpen((current) => (current?.level === level ? null : current));
    },
    step: (dir: -1 | 1) => {
      const next = open ? LEVELS[LEVELS.indexOf(open.level) + dir] : undefined;
      if (next && segmentOf(bar.current, next)) show(next);
    },
    pick: (level: PathLevel, id: string, newTab: boolean, current: string) => {
      setOpen(null);
      const chosen = level === 'schema' ? id : (schema ?? current);
      setSchema(level === 'schema' ? id : null);
      if (choose(actions, level, { id, newTab, schema: chosen }))
        setWave(Date.now());
      if (level === 'schema')
        window.setTimeout(() => {
          show('object', id);
        }, CASCADE_MS);
    },
  };
}
