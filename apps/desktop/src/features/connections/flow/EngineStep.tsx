// SPDX-License-Identifier: Apache-2.0
import { useRef, useState } from 'react';
import { Button, EngineIcon, Icon, isEngineId, TextField } from '@openrows/ui';
import type { EngineDto, UrlDraft } from '@openrows/ipc';
import { engineName, isBetaEngine } from '../engines';
import type { EngineChoice } from '../engines';
import { ENGINE_COUNT, engineGroups, tileDetail } from './engineGroups';
import { FlowIcon } from './FlowIcon';
import { FormSection } from './FormSection';
import type { PasteUrl } from './usePasteUrl';
import styles from './Engine.module.css';
import page from './Flow.module.css';
import { t } from './messages';

/**
 * Step 1: paste a connection URL (the fast path to the test), or search and
 * pick one of the engine tiles; engines without a driver say "Coming soon".
 */
export function EngineStep(props: {
  engine: EngineDto | null;
  paste: PasteUrl;
  onPick: (engine: EngineDto) => void;
  onPaste: () => void;
}): React.JSX.Element {
  const [query, setQuery] = useState('');
  const groups = engineGroups(query);
  return (
    <>
      <h2 className={page.title}>{t('engineTitle')}</h2>
      <p className={page.lede}>{t('engineLede')}</p>
      <PasteField paste={props.paste} onGo={props.onPaste} />
      <TextField
        className={styles.search}
        aria-label={t('searchEngines')}
        type="search"
        controlSize="search"
        placeholder={t('searchCount', { count: String(ENGINE_COUNT) })}
        autoComplete="off"
        spellCheck={false}
        leadingIcon={<Icon name="search" />}
        value={query}
        onChange={(event) => {
          setQuery(event.target.value);
        }}
      />
      {groups.length === 0 && <p className={page.notice}>{t('noEngine')}</p>}
      {groups.map((group) => (
        <FormSection key={group.title} title={group.title} bare>
          <div className={styles.grid}>
            {group.choices.map((choice) => (
              <EngineTileButton
                key={choice.id}
                choice={choice}
                selected={
                  choice.engine !== null && choice.engine === props.engine
                }
                onPick={props.onPick}
              />
            ))}
          </div>
        </FormSection>
      ))}
    </>
  );
}

function EngineTileButton(props: {
  choice: EngineChoice;
  selected: boolean;
  onPick: (engine: EngineDto) => void;
}): React.JSX.Element {
  const { choice } = props;
  const engine = choice.engine;
  return (
    <button
      type="button"
      className={styles.tile}
      aria-pressed={props.selected}
      aria-disabled={!engine}
      onClick={() => {
        if (engine) props.onPick(engine);
      }}
    >
      {isEngineId(choice.id) && <EngineIcon engine={choice.id} size="lg" />}
      <span className={page.lines}>
        <span className={styles.name}>{choice.name}</span>
        <span className={page.sub}>
          {tileDetail(choice)}
          {engine && isBetaEngine(engine) ? ` · ${t('beta')}` : ''}
        </span>
      </span>
    </button>
  );
}

function PasteField(props: {
  paste: PasteUrl;
  onGo: () => void;
}): React.JSX.Element {
  const { paste } = props;
  const pasted = useRef(false);
  return (
    <>
      <div className={styles.paste}>
        <FlowIcon name="link" />
        <TextField
          className={styles.pasteField}
          aria-label={t('pasteUrl')}
          placeholder={t('pastePlaceholder')}
          autoComplete="off"
          spellCheck={false}
          autoFocus
          value={paste.text}
          onPaste={() => {
            pasted.current = true;
          }}
          onChange={(event) => {
            paste.edit(event.target.value, pasted.current);
            pasted.current = false;
          }}
          onKeyDown={(event) => {
            if (event.key !== 'Enter' || !paste.parsed) return;
            event.preventDefault();
            props.onGo();
          }}
        />
        <Button variant="primary" disabled={!paste.parsed} onClick={props.onGo}>
          {t('continue')}
        </Button>
      </div>
      <p className={styles.hit} role="status" data-invalid={paste.invalid}>
        {paste.parsed && <Detected parsed={paste.parsed} />}
        {paste.invalid && t('pasteInvalid')}
      </p>
    </>
  );
}

function Detected({ parsed }: { parsed: UrlDraft }): React.JSX.Element {
  const place = parsed.file
    ? parsed.file.path || parsed.host
    : `${parsed.host}:${String(parsed.port)}` +
      (parsed.database
        ? t('pasteDatabase', { database: parsed.database })
        : '');
  return (
    <>
      {isEngineId(parsed.engine) && (
        <EngineIcon engine={parsed.engine} size="sm" />
      )}
      <span>
        {t('pasteDetected', { engine: engineName(parsed.engine), place })}
      </span>
    </>
  );
}
