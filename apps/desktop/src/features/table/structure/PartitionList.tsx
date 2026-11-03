// SPDX-License-Identifier: Apache-2.0
import { DisclosureSection } from '@openrows/ui';
import type { PartitionDto } from '@openrows/ipc';
import { formatRows } from './format';
import styles from './Structure.module.css';
import { t } from '../messages/lazyStructure';

/** A partitioned table's partitions: name, bound and estimated rows. */
export function PartitionList({
  partitions,
}: {
  partitions: readonly PartitionDto[];
}): React.JSX.Element {
  return (
    <DisclosureSection
      label={t('partitions')}
      count={partitions.length}
      defaultOpen
    >
      <table className={styles.partitions} aria-label={t('partitions')}>
        <thead>
          <tr>
            <th>{t('name2')}</th>
            <th>{t('bound')}</th>
            <th>{t('estimatedRows')}</th>
          </tr>
        </thead>
        <tbody>
          {partitions.map((partition) => (
            <tr key={`${partition.schema}.${partition.name}`}>
              <td>
                {partition.partitioned
                  ? t('namePartitioned', { name: partition.name })
                  : partition.name}
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
