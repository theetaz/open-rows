// SPDX-License-Identifier: Apache-2.0
import { revealDatabase } from '@openrows/ipc';
import type { ProfileDto } from '@openrows/ipc';
import type { Connections } from './useConnections';

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
          state === 'connecting' ? 'Cancel connection' : 'Disconnect',
          () => {
            connections.disconnectProfile(profile.id);
          },
        ]
      : [
          'Connect',
          () => {
            connections.select(profile.id);
          },
        ],
    ...revealAction(profile),
    [
      'Connection settings…',
      () => {
        connections.setSheet({ kind: 'edit', profile });
      },
    ],
    [
      'Export connection…',
      () => {
        connections.setSheet({ kind: 'export', ids: [profile.id] });
      },
    ],
    ...(profile.group
      ? [
          [
            'Export group…',
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
      'Export all connections…',
      () => {
        connections.setSheet({ kind: 'export' });
      },
    ],
    [
      'Import connections…',
      () => {
        connections.setSheet({ kind: 'import' });
      },
    ],
    ['Duplicate', () => void connections.duplicate(profile)],
    [
      'Delete…',
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
          'Reveal in Finder / File Manager',
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
