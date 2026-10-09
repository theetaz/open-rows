// SPDX-License-Identifier: Apache-2.0
import { Button, Icon, IconButton, TextField } from '@openrows/ui';
import type { ExportFormat } from './exportImage';
import styles from './Erd.module.css';

type Props = {
  query: string;
  onQuery: (query: string) => void;
  /** Return in the search field: centre the next match. */
  onNext: () => void;
  matches: number;
  scale: number;
  onZoom: (factor: number) => void;
  onFit: () => void;
  exporting: boolean;
  onExport: (format: ExportFormat) => void;
  onDone: () => void;
};

const ZOOM_STEP = 1.25;

/** Search, zoom, fit and export controls above the canvas. */
export function DiagramToolbar(props: Props): React.JSX.Element {
  const { query } = props;
  return (
    <div className={styles.toolbar} role="toolbar" aria-label="Diagram">
      <TextField
        className={styles.search}
        type="search"
        aria-label="Find table"
        placeholder="Find table"
        leadingIcon={<Icon name="search" />}
        value={query}
        onChange={(event) => {
          props.onQuery(event.target.value);
        }}
        onKeyDown={(event) => {
          if (event.key !== 'Enter' || event.nativeEvent.isComposing) return;
          event.preventDefault();
          props.onNext();
        }}
      />
      <span className={styles.count} role="status">
        {query.trim()
          ? `${String(props.matches)} ${props.matches === 1 ? 'match' : 'matches'}`
          : ''}
      </span>
      <span className={styles.spacer} />
      <ZoomControls {...props} />
      <Button
        size="sm"
        disabled={props.exporting}
        onClick={() => {
          props.onExport('svg');
        }}
      >
        Export SVG…
      </Button>
      <Button
        size="sm"
        disabled={props.exporting}
        onClick={() => {
          props.onExport('png');
        }}
      >
        Export PNG…
      </Button>
      <Button size="sm" onClick={props.onDone}>
        Done
      </Button>
    </div>
  );
}

function ZoomControls(props: Props): React.JSX.Element {
  return (
    <>
      <IconButton
        aria-label="Zoom out"
        size="sm"
        variant="ghost"
        onClick={() => {
          props.onZoom(1 / ZOOM_STEP);
        }}
      >
        <Icon name="minus" />
      </IconButton>
      <span className={styles.zoom}>
        {`${String(Math.round(props.scale * 100))}%`}
      </span>
      <IconButton
        aria-label="Zoom in"
        size="sm"
        variant="ghost"
        onClick={() => {
          props.onZoom(ZOOM_STEP);
        }}
      >
        <Icon name="plus" />
      </IconButton>
      <IconButton
        aria-label="Fit to view"
        size="sm"
        variant="ghost"
        onClick={props.onFit}
      >
        <Icon name="fit" />
      </IconButton>
    </>
  );
}
