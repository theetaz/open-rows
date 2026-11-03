// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { Button, TextField } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import { listObjects } from '@openrows/ipc';
import type { DiagramTarget } from '@openrows/ipc';
import { t } from './messages';

/** Choose exact table names from the entire schema, independently of the diagram cap. */
export function TablePicker(props: {
  target: DiagramTarget;
  onSelect: (names: string[] | undefined) => void;
  onClose: () => void;
}): React.JSX.Element {
  const [names, setNames] = useState<string[]>([]);
  const [selected, setSelected] = useState(
    () => new Set(props.target.tables ?? []),
  );
  const [search, setSearch] = useState('');
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    const { profileId, database, schema } = props.target;
    listObjects(
      profileId,
      database,
      { schema, kinds: ['table'], limit: null },
      true,
    ).then(
      (reply) => {
        if (live) setNames(reply.data.map((table) => table.name));
      },
      (reason: unknown) => {
        if (live)
          setError(reason instanceof Error ? reason.message : String(reason));
      },
    );
    return () => {
      live = false;
    };
  }, [props.target]);
  return (
    <Sheet title={t('chooseDiagramTables')} onClose={props.onClose}>
      <TextField
        label={t('filterTables')}
        value={search}
        onChange={(event) => {
          setSearch(event.target.value);
        }}
      />
      <p>{t('countSelectedMaximum', { count: String(selected.size) })}</p>
      {error && <p role="alert">{error}</p>}
      <TableChoices
        key={search}
        names={names}
        search={search}
        selected={selected}
        onChange={setSelected}
      />
      <SheetActions>
        <Button
          onClick={() => {
            props.onSelect(undefined);
          }}
        >
          {t('wholeSchema')}
        </Button>
        <Button onClick={props.onClose}>{t('cancel')}</Button>
        <Button
          variant="primary"
          disabled={!selected.size || selected.size > 400}
          onClick={() => {
            props.onSelect([...selected]);
          }}
        >
          {t('showSelection')}
        </Button>
      </SheetActions>
    </Sheet>
  );
}

function TableChoices(props: {
  names: readonly string[];
  search: string;
  selected: ReadonlySet<string>;
  onChange: (names: Set<string>) => void;
}): React.JSX.Element {
  const { names, search, selected } = props;
  const [limit, setLimit] = useState(200);
  const shown = names.filter((name) =>
    name.toLowerCase().includes(search.toLowerCase()),
  );
  return (
    <div style={{ maxHeight: '45vh', overflow: 'auto' }}>
      {shown.slice(0, limit).map((name) => (
        <label key={name} style={{ display: 'block' }}>
          <input
            type="checkbox"
            checked={selected.has(name)}
            onChange={(event) => {
              const next = new Set(selected);
              if (event.target.checked) next.add(name);
              else next.delete(name);
              props.onChange(next);
            }}
          />{' '}
          {name}
        </label>
      ))}
      {shown.length > limit && (
        <Button
          size="sm"
          onClick={() => {
            setLimit((current) => current + 200);
          }}
        >
          {t('showMoreTables')}
        </Button>
      )}
    </div>
  );
}
