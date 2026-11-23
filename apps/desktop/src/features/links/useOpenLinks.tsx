// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import type { OpenLinkDto } from '@openrows/ipc';
import { Button } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import type { Connections } from '../connections';
import type { QueryTabs } from '../query';
import type { Catalog } from '../schema';
import { openTableView } from '../table/open';
import { seedFilters } from '../table/links/seeds';
import { tableLayoutKey } from '../../results/columnLayoutStore';
import { linkRows, resolveLink } from './openLink';
import type { ResolvedLink } from './openLink';
import { t } from './messages';
import styles from './Links.module.css';

type Input = { connections: Connections; tabs: QueryTabs; catalog: Catalog };

/** A schema to select once its connection and database are active. */
type WantedSchema = { profileId: string; database: string; schema: string };

/** Incoming links and the confirmation sheet they wait in. */
type OpenLinks = {
  request: (link: OpenLinkDto) => void;
  sheet: React.JSX.Element | null;
};

/**
 * Open a confirmed link: select (and so connect) its connection and
 * database, then show its schema or open its relation with the link's
 * parameterised filters. Nothing is executed beyond the usual table page.
 */
function applyLink(
  resolved: ResolvedLink,
  input: Pick<Input, 'connections' | 'tabs'>,
): WantedSchema | null {
  const { link, profile } = resolved;
  if (link.kind === 'connection') {
    input.connections.select(profile.id, link.database ?? undefined);
    return null;
  }
  const { database, schema, object } = link;
  input.connections.select(profile.id, database);
  if (object) {
    if (link.kind === 'table' && link.filters.length)
      seedFilters(
        tableLayoutKey({
          profileId: profile.id,
          database,
          schema,
          table: object.name,
        }),
        link.filters,
      );
    openTableView(
      input.tabs,
      { connectionId: profile.id, database },
      { database, schema, name: object.name, kind: object.kind },
    );
  }
  return { profileId: profile.id, database, schema };
}

/**
 * `openrows://` links (FR-APP-09): every link that names a connection waits
 * for the user's confirmation, with a preview of where it connects and what
 * it opens, before anything connects.
 */
export function useOpenLinks(input: Input): OpenLinks {
  const [pending, setPending] = useState<OpenLinkDto | null>(null);
  const wanted = useRef<WantedSchema | null>(null);
  const { catalog } = input;
  // Selecting the schema waits until the link's connection and database are active.
  useEffect(() => {
    const schema = wanted.current;
    if (
      schema &&
      catalog.profileId === schema.profileId &&
      catalog.database === schema.database
    ) {
      wanted.current = null;
      catalog.setSchema(schema.schema);
    }
  });
  const close = (): void => {
    setPending(null);
  };
  // A link that launched the app waits until the saved profiles are read.
  const resolved =
    pending && input.connections.profilesLoaded !== false
      ? resolveLink(pending, input.connections.profiles)
      : null;
  return {
    request: setPending,
    sheet: resolved && (
      <OpenLinkSheet
        pending={resolved}
        onCancel={close}
        onOpen={(resolved) => {
          close();
          wanted.current = applyLink(resolved, input);
        }}
      />
    ),
  };
}

function OpenLinkSheet(props: {
  pending: ResolvedLink | { error: string };
  onCancel: () => void;
  onOpen: (resolved: ResolvedLink) => void;
}): React.JSX.Element {
  const { pending } = props;
  if ('error' in pending)
    return (
      <Sheet title={t('cannotOpenLink')} size="alert" onClose={props.onCancel}>
        <p role="alert">{pending.error}</p>
        <SheetActions>
          <Button variant="primary" onClick={props.onCancel}>
            {t('ok')}
          </Button>
        </SheetActions>
      </Sheet>
    );
  return (
    <Sheet title={t('openLink')} onClose={props.onCancel}>
      <dl className={styles.details} aria-label={t('linkDetails')}>
        {linkRows(pending).map(([label, value], index) => (
          <div key={`${label}-${String(index)}`}>
            <dt>{label}</dt>
            <dd>{value}</dd>
          </div>
        ))}
      </dl>
      <p className={styles.note}>{t('openingConnectsConnection')}</p>
      <SheetActions>
        <Button onClick={props.onCancel}>{t('cancel')}</Button>
        <Button
          variant="primary"
          onClick={() => {
            props.onOpen(pending);
          }}
        >
          {t('connectOpen')}
        </Button>
      </SheetActions>
    </Sheet>
  );
}
