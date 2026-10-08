// SPDX-License-Identifier: Apache-2.0
import type { ProfileDto } from '@openrows/ipc';

const names = {
  local: 'Local',
  dev: 'Development',
  staging: 'Staging',
  prod: 'Production',
  custom: 'Custom',
};

/** Group by environment and optional saved folder; identical database names stay connection-scoped. */
export function workspaceGroups(profiles: readonly ProfileDto[]): {
  id: string;
  label: string;
  environment: ProfileDto['env']['kind'];
  profiles: ProfileDto[];
}[] {
  const groups = new Map<
    string,
    {
      id: string;
      label: string;
      environment: ProfileDto['env']['kind'];
      profiles: ProfileDto[];
    }
  >();
  for (const profile of profiles) {
    const environment = profile.env.kind;
    const label = profile.env.label ?? names[environment];
    const id = JSON.stringify([environment, label, profile.group]);
    const group = groups.get(id) ?? {
      id,
      label: profile.group ? `${label} · ${profile.group}` : label,
      environment,
      profiles: [],
    };
    group.profiles.push(profile);
    groups.set(id, group);
  }
  return [...groups.values()];
}

/** Profiles in the order the rail shows them; ⌘1–⌘9 follow this order. */
export function railOrder(profiles: readonly ProfileDto[]): ProfileDto[] {
  return workspaceGroups(profiles).flatMap((group) => group.profiles);
}

/** ⌘ shortcut of the profile at a rail position, for the first nine. */
export function railShortcut(index: number): string | undefined {
  return index < 9 ? `⌘${String(index + 1)}` : undefined;
}
