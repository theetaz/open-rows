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
import { SavedQueriesSection, SidebarQueries } from './SidebarQueries';
import type { SidebarQueryProps } from './SidebarQueries';
import type { Connections } from '../connections';
import type { InspectionTarget } from '../inspection/target';
import { ObjectActions } from './ObjectActions';
import type { ObjectMenu, SessionLookup } from './ObjectActions';
import { SchemaDumpMenu } from './SchemaDumpMenu';
import type { JobActions } from './JobActions';
import { CreateMenu } from './CreateMenu';
import type { ObjectEditing } from './useObjectEditing';
import { isLinkable } from '../links/linkText';
import { linkCopier, schemaLinkCopier } from './sidebarLinks';
import styles from './Sidebar.module.css';
import { t } from './messages';

/** The sidebar's views: catalog objects, saved queries and history. */
const sidebarSegments = [
  { value: 'Items', label: t('items') },
  { value: 'Queries', label: t('queries') },
  { value: 'History', label: t('history') },
];
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
        {props.header}
        <TextField
          controlSize="search"
          leadingIcon={<Icon name="search" />}
          aria-label={t('searchItems')}
          placeholder={searchPlaceholder(segment)}
          trailing={segment === 'Items' && <SidebarFilters catalog={catalog} />}
          value={search}
          onChange={(event) => {
            setSearch(event.target.value);
          }}
        />
        <SegmentedControl
          aria-label={t('sidebarMode')}
          options={sidebarSegments}
          value={segment}
          onValueChange={setSegment}
          size="compact"
          variant="source"
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
          onCreate={props.objects.features ? setCreate : undefined}
        />
      </GlassPanel>
      {schemaMenu && props.jobs && (
        <SchemaDumpMenu
          {...schemaMenu}
          catalog={catalog}
          jobs={props.jobs}
          onCopyLink={schemaLinkCopier(props.connections, catalog.database)}
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
  if (segment === 'History') return t('searchHistory');
  return segment === 'Queries' ? t('searchQueries') : t('searchTablesViews');
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
        engine={catalog.engine}
        onClose={props.onCloseMenu}
        sessionFor={props.sessionFor}
        onOpen={props.onOpen}
        onDiagram={props.onDiagram}
        onImport={props.onImport}
        features={objects.features}
        onEditor={objects.open}
        onCopyLink={(object) => {
          if (isLinkable(object.kind)) linkCopier(props.connections)?.(object);
        }}
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
    onCreate: ((point: MenuPoint) => void) | undefined;
  },
): React.JSX.Element {
  const { catalog } = props;
  return (
    <div className={styles.objects}>
      {props.segment === 'Items' && (
        <>
          {catalog.database && (
            <SchemaPicker
              catalog={catalog}
              onMenu={(point) => {
                props.onSchemaMenu(catalog.schema, point);
              }}
              onCreate={props.onCreate}
            >
              {props.schemaTools}
            </SchemaPicker>
          )}
          <ObjectTree
            catalog={catalog}
            search={props.search}
            onInspect={props.onInspect}
            onOpen={props.onOpen}
            onMenu={props.onMenu}
            onSchemaMenu={props.onSchemaMenu}
          />
          <SavedQueriesSection
            {...props}
            active={props.connections.active}
            onNew={() => {
              props.onOpenSql('');
            }}
          />
        </>
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
