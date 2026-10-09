// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { Button, TextField } from '@openrows/ui';
import { dumpTools, writeSettingsFile } from '@openrows/ipc';
import type { DumpToolDto, DumpToolKindDto } from '@openrows/ipc';
import { SettingRow } from '../appearance/InterfaceSettings';
import { settingsText } from './settingsFile';
import { setToolPath, TOOL_KINDS, toolSettings } from './toolSettings';
import styles from '../appearance/Appearance.module.css';

// A hook name keeps React Compiler from memoizing the store's hook call.
const useToolPaths = toolSettings.use;

const NAMES: Readonly<Record<DumpToolKindDto, string>> = {
  pgDump: 'pg_dump',
  pgRestore: 'pg_restore',
  psql: 'psql',
  mysqldump: 'mysqldump',
  mysql: 'mysql',
  aws: 'aws',
  sessionManagerPlugin: 'session-manager-plugin',
  kubectl: 'kubectl',
  cloudSqlProxy: 'cloud-sql-proxy',
  alloydbAuthProxy: 'alloydb-auth-proxy',
  cloudflared: 'cloudflared',
};

type Detected =
  | { status: 'loading' }
  | { status: 'ready'; tools: readonly DumpToolDto[] }
  | { status: 'failed'; message: string };

/** What the core finds for every tool; re-read after `revision` changes. */
function useDetected(revision: number): Detected {
  const [detected, setDetected] = useState<Detected>({ status: 'loading' });
  useEffect(() => {
    let live = true;
    dumpTools().then(
      (tools) => {
        if (live) setDetected({ status: 'ready', tools });
      },
      (error: unknown) => {
        const message = error instanceof Error ? error.message : String(error);
        if (live) setDetected({ status: 'failed', message });
      },
    );
    return () => {
      live = false;
    };
  }, [revision]);
  return detected;
}

/** One line on the copy in use: path and version, or why there is none. */
function toolSummary(tool: DumpToolDto | undefined): string {
  if (!tool) return 'Not checked yet.';
  if (tool.error) return tool.error;
  const found = `${tool.path ?? tool.name} (${tool.version ?? 'unknown version'})`;
  const source = tool.fromSettings ? 'Set here' : 'Found';
  const others = tool.others.length
    ? ` Also installed: ${tool.others.join(', ')}.`
    : '';
  return `${source}: ${found}.${others}`;
}

function ToolRow(props: {
  kind: DumpToolKindDto;
  path: string;
  tool: DumpToolDto | undefined;
  onSaved: () => void;
}): React.JSX.Element {
  const [draft, setDraft] = useState(props.path);
  const name = NAMES[props.kind];
  const save = (): void => {
    if (draft.trim() === props.path) return;
    setToolPath(props.kind, draft);
    props.onSaved();
  };
  return (
    <SettingRow title={name} description={toolSummary(props.tool)}>
      <TextField
        aria-label={`${name} path`}
        placeholder="Automatic"
        spellCheck={false}
        value={draft}
        onChange={(event) => {
          setDraft(event.target.value);
        }}
        onBlur={save}
        onKeyDown={(event) => {
          if (event.key === 'Enter') save();
        }}
      />
    </SettingRow>
  );
}

/**
 * Settings ▸ Tools: the `pg_dump`, `pg_restore`, `psql`, `mysqldump` and
 * `mysql` copies plus cloud route helpers, with detected versions.
 * A path set here wins over the installed copy that matches the server.
 */
export function ToolsPane(): React.JSX.Element {
  const paths = useToolPaths();
  const [revision, setRevision] = useState(0);
  const detected = useDetected(revision);
  const recheck = (): void => {
    setRevision((value) => value + 1);
  };
  const tools = detected.status === 'ready' ? detected.tools : [];
  return (
    <section aria-label="Client tools">
      <h3>Client tools</h3>
      <div className={styles.settingsGroup}>
        {TOOL_KINDS.map((kind) => (
          <ToolRow
            // A new saved path (or an external edit) restarts the draft.
            key={`${kind}:${paths[kind]}`}
            kind={kind}
            path={paths[kind]}
            tool={tools.find((tool) => tool.tool === kind)}
            onSaved={() => {
              // The core reads the paths from settings.json: write it now
              // rather than after the debounce, then probe again.
              writeSettingsFile(settingsText())
                .catch(() => undefined)
                .finally(recheck);
            }}
          />
        ))}
        <SettingRow
          title="Find tools again"
          description="Search the usual install folders and PATH again, for example after installing a newer version. Leave a path empty to use the installed copy that best matches each server."
        >
          <Button onClick={recheck}>Check again</Button>
        </SettingRow>
        {detected.status === 'failed' && <p role="alert">{detected.message}</p>}
      </div>
    </section>
  );
}
