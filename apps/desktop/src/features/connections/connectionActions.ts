// SPDX-License-Identifier: Apache-2.0
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
          'Disconnect',
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
    [
      'Connection settings…',
      () => {
        connections.setSheet({ kind: 'edit', profile });
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
