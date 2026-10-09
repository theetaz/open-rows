// SPDX-License-Identifier: Apache-2.0
import { DisclosureSection } from '@openrows/ui';
import type { PartitionDto } from '@openrows/ipc';
import { formatRows } from './format';
import styles from './Structure.module.css';

/** A partitioned table's partitions: name, bound and estimated rows. */
export function PartitionList({
  partitions,
}: {
  partitions: readonly PartitionDto[];
}): React.JSX.Element {
  return (
    <DisclosureSection label="Partitions" count={partitions.length} defaultOpen>
      <table className={styles.partitions} aria-label="Partitions">
        <thead>
          <tr>
            <th>name</th>
            <th>bound</th>
            <th>estimated_rows</th>
          </tr>
        </thead>
        <tbody>
          {partitions.map((partition) => (
            <tr key={`${partition.schema}.${partition.name}`}>
              <td>
                {partition.name}
                {partition.partitioned && ' (partitioned)'}
              </td>
              <td>{partition.bound}</td>
              <td>{formatRows(partition.rowEstimate)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </DisclosureSection>
  );
}
