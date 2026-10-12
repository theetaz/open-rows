// SPDX-License-Identifier: Apache-2.0
import { useCallback, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { GlassPanel, Icon, SegmentedControl, TextField } from '@openrows/ui';
import {
  useSidebarPreferences,
  writeSidebarPreferences,
} from '../schema/sidebarPreferences';
import { ObjectTree, SchemaPicker, SidebarFilters } from '../schema';
import type { Catalog, MenuPoint, ObjectRef } from '../schema';
import { HistoryList } from '../query';
import { SidebarQueries } from './SidebarQueries';
import type { SidebarQueryProps } from './SidebarQueries';
import type { Connections } from '../connections';
import type { InspectionTarget } from '../inspection/target';
import { ObjectActions } from './ObjectActions';
import type { ObjectMenu, SessionLookup } from './ObjectActions';
import { SchemaDumpMenu } from './SchemaDumpMenu';
import type { JobActions } from './JobActions';
import { CreateMenu } from './CreateMenu';
import type { ObjectEditing } from './useObjectEditing';
import styles from './Sidebar.module.css';

/** The sidebar's views: catalog objects, saved queries and history. */
const sidebarSegments = ['Items', 'Queries', 'History'].map((label) => ({
  value: label,
  label,
}));
type Props = SidebarQueryProps & {
  catalog: Catalog;
  jobs?: JobActions;
  onInspect: (target: InspectionTarget) => void;
  header: ReactNode;
  /** Engine tools beside the schema picker. */
  schemaTools?: ReactNode;
  /** The saved connections; the History segment lists their statements. */
  connections: Connections;
  /** Open an object; `preview` for a single click in the tree. */
  onOpen: (object: ObjectRef, preview?: boolean) => void;
  /** Open a history statement in a new query tab. */
  onOpenSql: (sql: string) => void;
  /** Sessions for exporting a table from its context menu. */
  sessionFor: SessionLookup;
  /** Show the ER diagram of an object's schema, centred on it. */
  onDiagram: (object: ObjectRef) => void;
  /** Import a file into a table. */
  onImport: (object: ObjectRef) => void;
  /** Object editors opened from the context and create menus. */
  objects: Pick<ObjectEditing, 'features' | 'open'>;
};
const exact = (event: React.KeyboardEvent, key: string): boolean =>
  event.metaKey &&
  event.key.toLowerCase() === key &&
  !event.altKey &&
  !event.ctrlKey &&
  !event.shiftKey &&
  !event.nativeEvent.isComposing;
/** Object tree of the active database, saved queries and history. */
export function WorkspaceSidebar(props: Props): React.JSX.Element {
  const { catalog } = props;
  const [segment, setSegment] = useState('Items');
  const [search, setSearch] = useSidebarSearch(catalog.profileId, segment);
  const { menu, setMenu, closeMenu } = useObjectMenuState();
  const [schemaMenu, setSchemaMenu] = useState<{
    schema: string;
    point: MenuPoint;
  } | null>(null);
  const [create, setCreate] = useState<MenuPoint | null>(null);
  const closeCreate = useCallback(() => {
    setCreate(null);
  }, []);
  const keyboard = useSidebarKeyboard(catalog, setSearch);
  return (
    <div {...keyboard} className={styles.container}>
      <GlassPanel className={styles.sidebar}>
        <SegmentedControl
          aria-label="Sidebar mode"
          options={sidebarSegments}
          value={segment}
          onValueChange={setSegment}
          className={styles.modes}
        />
        {props.header}
        <TextField
          className={styles.search}
          leadingIcon={<Icon name="search" />}
          aria-label="Search items"
          placeholder={searchPlaceholder(segment)}
          trailing={segment === 'Items' && <SidebarFilters catalog={catalog} />}
          value={search}
          onChange={(event) => {
            setSearch(event.target.value);
          }}
        />
        <SidebarItems
          {...props}
          segment={segment}
          search={search}
          onSchemaMenu={(schema, point) => {
            setSchemaMenu({ schema, point });
          }}
          onMenu={(object, point) => {
            setMenu({ object, point });
          }}
        />
        {catalog.database && (
          <SchemaPicker
            catalog={catalog}
            onMenu={(point) => {
              setSchemaMenu({ schema: catalog.schema, point });
            }}
            onCreate={props.objects.features ? setCreate : undefined}
          >
            {props.schemaTools}
          </SchemaPicker>
        )}
      </GlassPanel>
      {schemaMenu && props.jobs && (
        <SchemaDumpMenu
          {...schemaMenu}
          catalog={catalog}
          jobs={props.jobs}
          onClose={() => {
            setSchemaMenu(null);
          }}
        />
      )}
      <SidebarMenus
        {...props}
        menu={menu}
        create={create}
        onCloseMenu={closeMenu}
        onCloseCreate={closeCreate}
      />
    </div>
  );
}

const searchPlaceholder = (segment: string): string => {
  if (segment === 'History') return 'Search history…';
  return segment === 'Queries' ? 'Search queries…' : 'Search tables, views…';
};

/** An object's context menu and the `+` create menu, when open. */
function SidebarMenus(
  props: Props & {
    menu: ObjectMenu | null;
    create: MenuPoint | null;
    onCloseMenu: () => void;
    onCloseCreate: () => void;
  },
): React.JSX.Element {
  const { catalog, objects, create } = props;
  return (
    <>
      <ObjectActions
        menu={props.menu}
        onClose={props.onCloseMenu}
        sessionFor={props.sessionFor}
        onOpen={props.onOpen}
        onDiagram={props.onDiagram}
        onImport={props.onImport}
        features={objects.features}
        onEditor={objects.open}
        onBackup={(object) => {
          props.jobs?.backup(object.database, {
            schemas: [],
            tables: [{ schema: object.schema, name: object.name }],
          });
        }}
      />
      {create && objects.features && (
        <CreateMenu
          point={create}
          place={{ database: catalog.database, schema: catalog.schema }}
          features={objects.features}
          onChoose={objects.open}
          onClose={props.onCloseCreate}
        />
      )}
    </>
  );
}

/** The chosen sidebar segment: object tree, saved queries or history. */
function SidebarItems(
  props: Props & {
    segment: string;
    search: string;
    onMenu: (object: ObjectRef, point: MenuPoint) => void;
    onSchemaMenu: (schema: string, point: MenuPoint) => void;
  },
): React.JSX.Element {
  return (
    <div className={styles.objects}>
      {props.segment === 'Items' && (
        <ObjectTree
          catalog={props.catalog}
          search={props.search}
          onInspect={props.onInspect}
          onOpen={props.onOpen}
          onMenu={props.onMenu}
          onSchemaMenu={props.onSchemaMenu}
        />
      )}
      {props.segment === 'Queries' && (
        <SidebarQueries
          {...props}
          active={props.connections.active}
          profiles={props.connections.profiles}
        />
      )}
      {props.segment === 'History' && (
        <HistoryList
          active={props.connections.active}
          connections={props.connections.profiles}
          text={props.search}
          onOpen={props.onOpenSql}
        />
      )}
    </div>
  );
}

function useSidebarSearch(
  profileId: string,
  segment: string,
): [string, (text: string) => void] {
  const [otherSearch, setOtherSearch] = useState('');
  const preferences = useSidebarPreferences(profileId);
  return [
    segment === 'Items' ? preferences.text : otherSearch,
    (text) => {
      if (segment === 'Items' && profileId)
        writeSidebarPreferences(profileId, { text });
      else setOtherSearch(text);
    },
  ];
}

function useSidebarKeyboard(
  catalog: Catalog,
  setSearch: (text: string) => void,
) {
  const ref = useRef<HTMLDivElement>(null);
  return {
    ref,
    onKeyDown: (event: React.KeyboardEvent<HTMLDivElement>): void => {
      if (event.key === 'Escape') {
        event.preventDefault();
        setSearch('');
      } else if (exact(event, 'f')) {
        event.preventDefault();
        ref.current?.querySelector('input')?.focus();
      } else if (exact(event, 'r')) {
        event.preventDefault();
        catalog.refresh();
      }
    },
  };
}

function useObjectMenuState() {
  const [menu, setMenu] = useState<ObjectMenu | null>(null);
  const closeMenu = useCallback(() => {
    setMenu(null);
  }, []);
  return { menu, setMenu, closeMenu };
}
