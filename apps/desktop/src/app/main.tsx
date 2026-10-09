// SPDX-License-Identifier: Apache-2.0
import { createRoot } from 'react-dom/client';
import { App } from './App';
import { startIpcSpike } from '../spike/bootstrap';
import { hydrateSettings } from '../features/settings';
import { installDiagnostics } from '../features/diagnostics';
import '@openrows/ui/tokens.css';
import './global.css';

const root = document.getElementById('root');
if (!root) throw new Error('Missing application root');
// Errors reach the log from the first line on; dev-only timing monitors start here too.
installDiagnostics();
// settings.json loads into the stores before the first render reads them.
void hydrateSettings().finally(() => {
  createRoot(root).render(<App />);
  void startIpcSpike();
});
