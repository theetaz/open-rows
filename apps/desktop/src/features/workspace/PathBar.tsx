// SPDX-License-Identifier: Apache-2.0
import { Fragment, useEffect, useRef } from 'react';
import type { CSSProperties } from 'react';
import { Icon, Picker } from '@openrows/ui';
import type { PickerSection } from '@openrows/ui';
import type { EnvDto, ProfileDto } from '@openrows/ipc';
import { environmentColour, statusLabel } from '../connections';
import { pathSegments } from './pathModel';
import type { PathLevel, PathSegment } from './pathModel';
import {
  EngineTile,
  EnvTag,
  connectionSections,
  databaseSections,
  objectSections,
  schemaSections,
} from './pathPickers';
import { usePathPicker } from './usePathPicker';
import type { PathActions } from './usePathPicker';
import styles from './PathBar.module.css';
import { t } from './navMessages';

type Props = PathActions & {
  /** Health, safety, favourite and refresh at the bar's end. */
  tools: React.ReactNode;
};

const LEVEL_NAMES: Readonly<Record<PathLevel, () => string>> = {
  connection: () => t('connection'),
  database: () => t('database'),
  schema: () => t('schema'),
  object: () => t('object'),
};

function Segment(props: {
  segment: PathSegment;
  expanded: boolean;
  onToggle: (level: PathLevel) => void;
}): React.JSX.Element {
  const { segment } = props;
  return (
    <button
      type="button"
      className={styles.segment}
      data-level={segment.level}
      data-role={segment.role}
      aria-haspopup="dialog"
      aria-expanded={props.expanded}
      aria-label={t('segmentLabel', {
        level: LEVEL_NAMES[segment.level](),
        name: segment.text,
      })}
      title={segment.title ?? segment.text}
      onClick={() => {
        props.onToggle(segment.level);
      }}
    >
      {segment.engine ? (
        <EngineTile engine={segment.engine} size="sm" />
      ) : (
        segment.icon && <Icon name={segment.icon} />
      )}
      <span key={segment.text} className={styles.swap}>
        {segment.text}
      </span>
      {segment.role === 'prompt' && (
        <kbd className={styles.key}>{t('locationKey')}</kbd>
      )}
    </button>
  );
}

/** The picker of the open segment. */
function sectionsOf(
  level: PathLevel,
  actions: PathActions,
  current: Readonly<Record<PathLevel, string | undefined>>,
): PickerSection[] {
  const { connections, catalog } = actions;
  if (level === 'connection')
    return connectionSections(connections, connections.active?.id);
  if (level === 'database')
    return databaseSections(catalog, current.database ?? '');
  if (level === 'schema') return schemaSections(catalog, current.schema ?? '');
  return objectSections(
    catalog,
    current.schema ?? catalog.schema,
    current.object,
  );
}

function placeholderOf(
  level: PathLevel,
  current: Readonly<Record<PathLevel, string | undefined>>,
): string {
  if (level === 'connection') return t('switchConnection');
  if (level === 'database')
    return t('databasesOn', { name: current.connection ?? '' });
  if (level === 'schema')
    return t('schemasIn', { database: current.database ?? '' });
  return t('tablesIn', { schema: current.schema ?? '' });
}

/** Loud environments outline (staging) or tint (prod) the bar. */
function envStyle(env: EnvDto | undefined): CSSProperties | undefined {
  return env?.kind === 'custom'
    ? ({ '--env-color': environmentColour(env) } as CSSProperties)
    : undefined;
}

/** The address of the shown tab, from its connection, database and schema. */
function segmentsOf(
  props: Props,
  profile: ProfileDto | undefined,
): PathSegment[] {
  return pathSegments({
    tab: props.tab,
    profile,
    database: props.catalog.database,
    schema: props.catalog.schema,
    ...(profile
      ? { status: statusLabel(props.connections.statusOf(profile.id)) }
      : {}),
  });
}

/** The chosen name of each level; a schema just picked wins over the path's. */
function currentOf(
  segments: readonly PathSegment[],
  schema: string | null,
): Record<PathLevel, string | undefined> {
  const current = Object.fromEntries(
    segments
      .filter((item) => !item.role || item.role === 'last')
      .map((item) => [item.level, item.text]),
  ) as Record<PathLevel, string | undefined>;
  if (schema) current.schema = schema;
  return current;
}

/**
 * The address of the shown tab (DESIGN.md §6): every segment opens a picker
 * for its level; ⌘L opens the last one, ← → in an empty search walks the
 * segments.
 */
export function PathBar(props: Props): React.JSX.Element {
  const bar = useRef<HTMLDivElement>(null);
  const profile =
    props.tab?.kind === 'connection' ? undefined : props.connections.active;
  const segments = segmentsOf(props, profile);
  const picker = usePathPicker(bar, segments, props);
  useEffect(() => {
    bar.current?.setAttribute('data-ready', 'true');
  }, []);
  const current = currentOf(segments, picker.schema);
  const env = profile?.env;
  return (
    <div
      ref={bar}
      className={styles.bar}
      role="group"
      aria-label={t('pathBar')}
      data-env={env?.kind}
      style={envStyle(env)}
    >
      {env && (
        <EnvTag
          env={env}
          className={
            env.kind === 'custom' || env.label ? styles.custom : styles.env
          }
        />
      )}
      {segments.map((segment, index) => (
        <Fragment key={segment.level}>
          {index > 0 && (
            <Icon name="chevronRight" className={styles.separator} />
          )}
          <Segment
            segment={segment}
            expanded={picker.open?.level === segment.level}
            onToggle={picker.toggle}
          />
        </Fragment>
      ))}
      <span className={styles.grow} data-tauri-drag-region />
      <div className={styles.tools}>{props.tools}</div>
      {picker.open && (
        <Picker
          key={picker.open.level}
          anchor={picker.open.anchor}
          placeholder={placeholderOf(picker.open.level, current)}
          sections={sectionsOf(picker.open.level, props, current)}
          wide={picker.open.level === 'connection'}
          dx={8}
          onStep={picker.step}
          onPick={(item, options) => {
            if (picker.open)
              picker.pick(
                picker.open.level,
                item.id,
                options.newTab,
                current.schema ?? '',
              );
          }}
          onClose={() => {
            if (picker.open) picker.close(picker.open.level);
          }}
        />
      )}
    </div>
  );
}
