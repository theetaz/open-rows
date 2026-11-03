// SPDX-License-Identifier: Apache-2.0
import { defineMessages } from '@openrows/ui/i18n';

export const { t } = defineMessages({
  new: 'New',
  sameNameExists: 'Same name exists',
  identicalSkip: 'Identical (skip)',
  skip: 'Skip',
  replace: 'Replace',
  keepBothSuffix: 'Keep both with suffix',
  actionName: 'Action for {name}',
  savePasswordName: 'Save password for {name} in keychain',
  noConnectionsFound: 'No connections found.',
  importedCommandsUntrusted:
    'Imported commands are untrusted. Their exact program and arguments must be confirmed before execution.',
  openrowsJsonFile: 'OpenRows JSON file…',
  pgpassDefaultPgpassfile: 'pgpass (default / PGPASSFILE)',
  pgServiceDefault: 'pg_service (default / environment)',
  mysqlMyCnf: 'MySQL ~/.my.cnf',
  exportConnections: 'Export connections',
  exportsSettingsFile:
    'Exports settings and file paths. Passwords, passphrases, private keys and command output are excluded.',
  selectAll: 'Select all',
  cancel: 'Cancel',
  export: 'Export…',
  importConnections: 'Import connections',
  source: 'Source',
  previewFirstNothing: 'Preview first. Nothing is saved until Confirm import.',
  preview: 'Preview…',
  confirmImport: 'Confirm import',
  selectGroupGroup: 'Select group: {group}',
});
