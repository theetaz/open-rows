// SPDX-License-Identifier: Apache-2.0
import { revealDatabase } from '@openrows/ipc';
import type { ProfileDto } from '@openrows/ipc';
import type { Connections } from './useConnections';
import { connectionLink, copyLink } from '../links/linkText';
import { t } from './messages/profile';

/** A labelled connection command for row menus. */
export type ConnectionAction = readonly [label: string, run: () => void];

/** Connect or disconnect, settings, duplicate and delete for one profile. */
export function connectionActions(
  profile: ProfileDto,
  connections: Connections,
): readonly ConnectionAction[] {
  const state = connections.statusOf(profile.id).state;
  const live = state === 'connected' || state === 'connecting';
  return [
    live
      ? [
          state === 'connecting' ? t('cancelConnection') : t('disconnect'),
          () => {
            connections.disconnectProfile(profile.id);
          },
        ]
      : [
          t('connect'),
          () => {
            connections.select(profile.id);
          },
        ],
    ...revealAction(profile),
    [
      t('copyLink'),
      () => {
        copyLink(connectionLink(profile)).catch(() => undefined);
      },
    ],
    [
      t('connectionSettings'),
      () => {
        connections.setSheet({ kind: 'edit', profile });
      },
    ],
    [
      t('exportConnection'),
      () => {
        connections.setSheet({ kind: 'export', ids: [profile.id] });
      },
    ],
    ...(profile.group
      ? [
          [
            t('exportGroup'),
            () => {
              connections.setSheet({
                kind: 'export',
                ids: connections.profiles
                  .filter((item) => item.group === profile.group)
                  .map((item) => item.id),
              });
            },
          ] as const,
        ]
      : []),
    [
      t('exportAllConnections'),
      () => {
        connections.setSheet({ kind: 'export' });
      },
    ],
    [
      t('importConnections'),
      () => {
        connections.setSheet({ kind: 'import' });
      },
    ],
    [t('duplicate'), () => void connections.duplicate(profile)],
    [
      t('delete'),
      () => {
        connections.confirmRemove(profile);
      },
    ],
  ];
}

/** Reveal failures remain visible in the workspace's native file notice. */
function revealAction(profile: ProfileDto): readonly ConnectionAction[] {
  return profile.file?.path
    ? [
        [
          t('revealFinderFile'),
          () => {
            revealDatabase(profile.id).catch((error: unknown) => {
              window.dispatchEvent(
                new CustomEvent('openrows:file-error', {
                  detail:
                    error instanceof Error ? error.message : String(error),
                }),
              );
            });
          },
        ] as const,
      ]
    : [];
}
