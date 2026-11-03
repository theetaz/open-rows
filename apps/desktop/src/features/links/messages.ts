// SPDX-License-Identifier: Apache-2.0
import { defineMessages } from '@openrows/ui/i18n';

export const { t } = defineMessages({
  severalSavedConnections:
    "Several saved connections are named “{name}”. Copy the link again in OpenRows so it includes the connection's id.",
  noSavedConnection:
    'No saved connection matches this link. Add or import the connection first.',
  connection: 'Connection',
  server: 'Server',
  database: 'Database',
  schema: 'Schema',
  opens: 'Opens',
  filter: 'Filter',
  cannotOpenLink: 'Cannot open link',
  ok: 'OK',
  openLink: 'Open link?',
  linkDetails: 'Link details',
  openingConnectsConnection:
    "Opening connects with this connection's saved settings and safety policy. Links never run SQL.",
  cancel: 'Cancel',
  connectOpen: 'Connect and Open',
});
