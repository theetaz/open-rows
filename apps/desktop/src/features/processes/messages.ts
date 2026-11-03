// SPDX-License-Identifier: Apache-2.0
import { defineMessages } from '@openrows/ui/i18n';

export const { t } = defineMessages({
  list: 'This list',
  connectionReadOnly: 'This connection is read-only',
  copyQuery: 'Copy Query',
  cancelQuery: 'Cancel Query',
  killProcess: 'Kill Process',
  actionsProcess: 'Actions for {process}',
  processes: 'Processes',
  cancel: 'Cancel',
  off: 'Off',
  valueS: '{value} s',
  filterProcesses: 'Filter processes',
  filterProcesses2: 'Filter processes…',
  autoRefresh: 'Auto-refresh',
  refreshProcesses: 'Refresh processes',
  loadingProcesses: 'Loading processes…',
  shownCountProcesses: '{shown} of {count} processes',
  processesDatabase: 'Processes on {database}',
  connectListServer: 'Connect to list the server’s processes.',
  pid: 'PID',
  user: 'User',
  database: 'Database',
  client: 'Client',
  application: 'Application',
  state: 'State',
  waiting: 'Waiting for',
  duration: 'Duration',
  queryStart: 'Query start',
  query: 'Query',
  processIdOwner: 'process {id} ({owner})',
  runningStatementStops:
    'Its running statement stops with an error; the session stays connected.',
  sessionDisconnectedAny:
    'Its session is disconnected and any open transaction is rolled back.',
  connectionProcessList:
    'This is the connection this process list runs on; the list reconnects on its next refresh.',
  oneOpenrowsOwn:
    'This is one of OpenRows’ own connections, possibly a query tab of yours.',
  cancelQueryLabel: 'Cancel the query of {label}?',
  killLabel: 'Kill {label}?',
  connectionReadOnlyCancelling:
    'This connection is read-only: cancelling queries and killing processes are disabled',
  processId: 'process {id}',
});
