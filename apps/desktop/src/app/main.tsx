// SPDX-License-Identifier: Apache-2.0
import { createRoot } from 'react-dom/client';
import { App } from './App';
import { startIpcSpike } from '../spike/bootstrap';
import '@openrows/ui/tokens.css';
import './global.css';

const root = document.getElementById('root');
if (!root) throw new Error('Missing application root');
createRoot(root).render(<App />);
void startIpcSpike();
