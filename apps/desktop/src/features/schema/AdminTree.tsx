// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { Button, DisclosureSection, Tree } from '@openrows/ui';
import type { ExtensionDto, RoleDto } from '@openrows/ipc';
import type { InspectionTarget } from '../inspection/target';
import type { InspectionCatalog } from './useInspectionCatalog';
import type { Resource } from './useResources';
import styles from './ObjectTree.module.css';

/** Database extensions and connection users/roles, fetched only on expand. */
export function AdminTree(props: {
  catalog: InspectionCatalog;
  onOpen: (target: InspectionTarget) => void;
  search: string;
}): React.JSX.Element | null {
  const features = props.catalog.capabilities;
  if (features.status !== 'ready') return null;
  return (
    <>
      {features.data.extensions && (
        <AdminSection {...props} kind="extensions" label="Extensions" />
      )}
      {features.data.roles && (
        <AdminSection {...props} kind="roles" label="Users & Roles" />
      )}
    </>
  );
}

function AdminSection(props: {
  catalog: InspectionCatalog;
  onOpen: (target: InspectionTarget) => void;
  search: string;
  kind: 'extensions' | 'roles';
  label: string;
}): React.JSX.Element {
  const [open, setOpen] = useState(false);
  const resource: Resource<(ExtensionDto | RoleDto)[]> =
    props.catalog[props.kind];
  const { load } = props.catalog;
  useEffect(() => {
    if (open && resource.status === 'idle') load(props.kind);
  }, [open, resource.status, load, props.kind]);
  const rows =
    resource.status === 'ready'
      ? resource.data.filter((row) =>
          row.name.toLowerCase().includes(props.search.toLowerCase()),
        )
      : [];
  return (
    <DisclosureSection
      label={props.label}
      defaultOpen={false}
      onOpenChange={setOpen}
    >
      {resource.status === 'loading' && (
        <p role="status" className={styles.note}>
          Loading…
        </p>
      )}
      {resource.status === 'error' && (
        <p role="alert" className={styles.error}>
          {resource.message}
        </p>
      )}
      {resource.status === 'ready' && !rows.length && (
        <p className={styles.note}>No {props.label.toLowerCase()}</p>
      )}
      <Tree label={props.label}>
        {rows.map((row) => (
          <AdminRow
            key={'scope' in row ? `${row.scope}\n${row.name}` : row.name}
            row={row}
            onOpen={props.onOpen}
          />
        ))}
      </Tree>
    </DisclosureSection>
  );
}

function AdminRow(props: {
  row: ExtensionDto | RoleDto;
  onOpen: (target: InspectionTarget) => void;
}): React.JSX.Element {
  const row = props.row;
  const role = 'scope' in row;
  const label = role ? `${row.name} @ ${row.scope}` : row.name;
  return (
    <li role="none">
      <Button
        size="sm"
        variant="ghost"
        role="treeitem"
        onClick={() => {
          props.onOpen(
            role
              ? { kind: 'role', name: row.name, scope: row.scope }
              : { kind: 'extension', name: row.name },
          );
        }}
      >
        {label}
      </Button>
    </li>
  );
}
