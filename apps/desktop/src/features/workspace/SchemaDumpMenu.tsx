// SPDX-License-Identifier: Apache-2.0
import { Icon } from '@openrows/ui';
import type { MenuEntry } from '@openrows/ui';
import { ContextMenu } from '../../shell/ContextMenu';
import type { Catalog, MenuPoint } from '../schema';
import type { JobActions } from './JobActions';
import { t } from './messages';

/** Schema actions reuse the database restore gate and preselect backup objects. */
export function SchemaDumpMenu(props: {
  schema: string;
  point: MenuPoint;
  catalog: Catalog;
  jobs: JobActions;
  /** Copy the schema's `openrows://` link; omitted without a saved connection. */
  onCopyLink?: ((schema: string) => void) | undefined;
  onClose: () => void;
}): React.JSX.Element {
  const { catalog, jobs, schema, onCopyLink } = props;
  const database = catalog.database;
  const offline = Boolean(catalog.offline);
  const entries: MenuEntry[] = [
    {
      kind: 'item',
      label: t('backUp'),
      icon: <Icon name={'database'} />,
      disabled: offline,
      run: () => {
        jobs.backup(database, { schemas: [schema], tables: [] });
      },
    },
    {
      kind: 'item',
      label: t('restore'),
      icon: <Icon name={'history'} />,
      disabled: offline,
      run: () => {
        jobs.restore(
          database,
          catalog.engine === 'postgres' ? schema : undefined,
        );
      },
    },
  ];
  const link: MenuEntry[] = onCopyLink
    ? [
        { kind: 'separator' },
        {
          kind: 'item',
          label: t('copyLink'),
          icon: <Icon name={'link'} />,
          run: () => {
            onCopyLink(schema);
          },
        },
      ]
    : [];
  return (
    <ContextMenu
      label={t('schemaActions', { schema })}
      at={props.point}
      target={props.point.target}
      entries={[...entries, ...link]}
      onClose={props.onClose}
    />
  );
}
