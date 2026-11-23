// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { vi } from 'vitest';
import { getProfile } from '@openrows/ipc';
import type { EngineDto, ProfileDto } from '@openrows/ipc';
import { ThemeProvider } from '@openrows/ui';
import { ConnectionSheets } from '../ConnectionSheets';
import type { ConnectionFlowRequest } from '../ConnectionSheets';
import type { Connections } from '../useConnections';
import { NewConnectionFlow } from './NewConnectionFlow';

type Mount = {
  /** Edit this saved profile (read through `getProfile`). */
  profile?: ProfileDto | undefined;
  /** A new connection of this engine, opened at Details. */
  engine?: EngineDto | undefined;
  /** A file opened from Finder. */
  filePath?: string;
  focus?: 'safety';
};

type Mounted = {
  user: ReturnType<typeof userEvent.setup>;
  onDone: ReturnType<typeof vi.fn>;
  onCancel: ReturnType<typeof vi.fn>;
  unmount: () => void;
};

/** Render the flow as a tab would; resolves once the form is on screen. */
export async function mountFlow(options: Mount = {}): Promise<Mounted> {
  const onDone = vi.fn();
  const onCancel = vi.fn();
  if (options.profile) vi.mocked(getProfile).mockResolvedValue(options.profile);
  const view = render(
    <ThemeProvider>
      <NewConnectionFlow
        mode={options.profile ? 'edit' : 'new'}
        {...(options.profile ? { profileId: options.profile.id } : {})}
        engine={options.engine}
        filePath={options.filePath}
        focus={options.focus}
        onDone={onDone}
        onCancel={onCancel}
      />
    </ThemeProvider>,
  );
  await screen.findByRole('form');
  return { user: userEvent.setup(), onDone, onCancel, unmount: view.unmount };
}

/** Click a step in the steps column, e.g. `Security`. */
export async function openStep(
  user: ReturnType<typeof userEvent.setup>,
  name: string,
): Promise<void> {
  const steps = screen.getByRole('navigation', { name: 'Steps' });
  await user.click(
    within(steps).getByRole('button', { name: new RegExp(`^\\d?\\s?${name}`) }),
  );
}

/** The footer's Continue button (the engine step's paste field has its own). */
export function continueButton(): HTMLElement {
  const buttons = screen.getAllByRole('button', { name: /^Continue/ });
  const last = buttons.at(-1);
  if (!last) throw new Error('no Continue button');
  return last;
}

type User = ReturnType<typeof userEvent.setup>;

/** Save: at once when editing; a new connection continues to its last step first. */
export async function saveFlow(user: User): Promise<void> {
  for (let step = 0; step < 5; step += 1) {
    if (screen.queryByRole('button', { name: 'Save' })) break;
    await user.click(continueButton());
  }
  await user.click(screen.getByRole('button', { name: 'Save' }));
}

/** Open Test and save and run the connection test. */
export async function runTest(user: User): Promise<void> {
  await openStep(user, 'Test and save');
  await user.click(
    screen.getByRole('button', { name: /^(Run test|Run again)$/ }),
  );
}

/** The live summary column. */
export function summary(): HTMLElement {
  return screen.getByRole('complementary', { name: 'Summary' });
}

/**
 * The connection sheets with the flow a new or edit request opens, hosted the
 * way the workspace's connection tab hosts it: done reloads and selects.
 */
export function SheetsWithFlow(props: {
  connections: Connections;
}): React.JSX.Element {
  const { connections } = props;
  const [flow, setFlow] = useState<ConnectionFlowRequest | null>(null);
  return (
    <>
      <ConnectionSheets connections={connections} onFlow={setFlow} />
      {flow && (
        <NewConnectionFlow
          {...flow}
          onDone={(profileId) => {
            setFlow(null);
            connections.reload();
            connections.select(profileId);
          }}
          onCancel={() => {
            setFlow(null);
          }}
        />
      )}
    </>
  );
}
