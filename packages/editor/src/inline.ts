// SPDX-License-Identifier: Apache-2.0
import { StateEffect, StateField } from '@codemirror/state';
import { Decoration, EditorView, WidgetType } from '@codemirror/view';
import type { DecorationSet } from '@codemirror/view';
import { editingRange } from './statement';
/** App-owned streaming transport. Cancellation must interrupt native work too. */
export type InlineGenerate = (input: {
  prompt: string;
  sql: string;
  signal: AbortSignal;
  delta: (text: string) => void;
}) => Promise<string>;
const replace = StateEffect.define<DecorationSet>();
const reviewField = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update: (value, transaction) => {
    if (transaction.docChanged) return Decoration.none;
    for (const effect of transaction.effects)
      if (effect.is(replace)) return effect.value;
    return value;
  },
  provide: (field) => EditorView.decorations.from(field),
});
/** A proposal targets exactly the original selection/statement, including an empty caret. */
export function inlineRange(view: EditorView): { from: number; to: number } {
  const { from, to, head } = view.state.selection.main;
  if (from !== to) return { from, to };
  const range = editingRange(view.state, head);
  return view.state.sliceDoc(range.from, range.to).trim()
    ? range
    : { from: head, to: head };
}
class ReviewWidget extends WidgetType {
  constructor(private readonly controller: Review) {
    super();
  }
  override toDOM(): HTMLElement {
    return this.controller.dom();
  }
  override ignoreEvent(): boolean {
    return true;
  }
}
class Review {
  readonly abort = new AbortController();
  readonly range;
  readonly original: string;
  private readonly document: string;
  private text = '';
  private proposal = '';
  private status: 'prompt' | 'running' | 'ready' | 'failed' = 'prompt';
  private failure = '';
  constructor(
    private readonly view: EditorView,
    private readonly generate: InlineGenerate,
  ) {
    this.range = inlineRange(view);
    this.original = view.state.sliceDoc(this.range.from, this.range.to);
    this.document = view.state.doc.toString();
  }
  show(): void {
    const marks = [
      Decoration.widget({
        widget: new ReviewWidget(this),
        block: true,
        side: 1,
      }).range(this.range.to),
    ];
    if (this.status !== 'prompt' && this.range.from !== this.range.to)
      marks.unshift(
        Decoration.mark({ class: 'cm-inline-original' }).range(
          this.range.from,
          this.range.to,
        ),
      );
    this.view.dispatch({ effects: replace.of(Decoration.set(marks, true)) });
  }
  cancel(): void {
    this.abort.abort();
    this.view.dispatch({ effects: replace.of(Decoration.none) });
    this.view.focus();
  }
  private accept(): void {
    if (
      this.status !== 'ready' ||
      this.abort.signal.aborted ||
      this.document !== this.view.state.doc.toString()
    )
      return;
    this.abort.abort();
    this.view.dispatch({
      changes: { ...this.range, insert: this.proposal },
      effects: replace.of(Decoration.none),
      userEvent: 'input.inline-proposal',
    });
    this.view.focus();
  }
  private async submit(prompt: string): Promise<void> {
    if (!prompt.trim() || this.status !== 'prompt') return;
    this.status = 'running';
    this.show();
    this.view.focus();
    try {
      const proposal = await this.generate({
        prompt,
        sql: this.original,
        signal: this.abort.signal,
        delta: (text) => {
          if (this.abort.signal.aborted) return;
          this.text = text.slice(0, 32768);
          this.show();
        },
      });
      if (this.abort.signal.aborted) return;
      if (!proposal.trim() || proposal.length > 16384)
        throw new Error('No bounded SQL proposal returned.');
      this.proposal = proposal;
      this.status = 'ready';
    } catch (error) {
      if (this.abort.signal.aborted) return;
      this.status = 'failed';
      this.failure =
        error instanceof Error ? error.message : 'Inline request failed.';
    }
    this.show();
  }
  dom(): HTMLElement {
    const host = document.createElement('section');
    host.className = 'cm-inline-review';
    host.setAttribute('aria-label', 'Inline SQL proposal');
    host.addEventListener('keydown', (event) => {
      if (event.key === 'Escape') {
        event.preventDefault();
        event.stopPropagation();
        this.cancel();
      }
    });
    if (this.status === 'prompt') this.prompt(host);
    else {
      const status = document.createElement('p');
      status.setAttribute(
        'role',
        this.status === 'failed' ? 'alert' : 'status',
      );
      status.textContent =
        this.failure ||
        (this.status === 'running'
          ? 'Generating SQL proposal… Escape cancels.'
          : 'Proposed SQL — review before running');
      const code = document.createElement('pre');
      code.setAttribute('aria-label', 'Proposed SQL');
      code.textContent = this.proposal || this.text;
      host.append(status, code);
      if (this.status === 'ready')
        host.append(
          button('Accept proposal', () => {
            this.accept();
          }),
        );
    }
    host.append(
      button(
        this.status === 'running' ? 'Cancel proposal' : 'Reject proposal',
        () => {
          this.cancel();
        },
      ),
    );
    return host;
  }
  private prompt(host: HTMLElement): void {
    const form = document.createElement('form');
    const label = document.createElement('label');
    label.textContent = 'Inline SQL prompt';
    const input = document.createElement('input');
    input.maxLength = 3500;
    input.setAttribute('aria-label', 'Inline SQL prompt');
    label.append(input);
    form.append(
      label,
      button('Generate proposal', () => {
        void this.submit(input.value);
      }),
    );
    form.addEventListener('submit', (event) => {
      event.preventDefault();
      void this.submit(input.value);
    });
    host.append(form);
    queueMicrotask(() => {
      if (input.isConnected) input.focus();
    });
  }
}
function button(label: string, action: () => void): HTMLButtonElement {
  const button = document.createElement('button');
  button.type = 'button';
  button.textContent = label;
  button.addEventListener('click', action);
  return button;
}
/** Install the lazy transport shortcut; document changes invalidate pending reviews. */
export function installInline(
  view: EditorView,
  generate: InlineGenerate,
  appKeys?: (event: KeyboardEvent) => boolean,
): { open: () => void; destroy: () => void } {
  let active: Review | null = null;
  view.dispatch({
    effects: StateEffect.appendConfig.of([
      reviewField,
      inlineTheme,
      EditorView.updateListener.of((update) => {
        if (!update.docChanged || !active) return;
        active.abort.abort();
        active = null;
      }),
    ]),
  });
  const open = (): void => {
    active?.cancel();
    const review = new Review(view, generate);
    active = review;
    review.abort.signal.addEventListener(
      'abort',
      () => {
        if (active === review) active = null;
      },
      { once: true },
    );
    review.show();
  };
  const key = (event: KeyboardEvent): void => {
    if (event.key === 'Escape' && active) {
      event.preventDefault();
      event.stopPropagation();
      active.cancel();
      active = null;
    } else if (
      (event.metaKey || event.ctrlKey) &&
      !event.shiftKey &&
      !event.altKey &&
      event.key.toLowerCase() === 'i'
    ) {
      if (appKeys) {
        if (appKeys(event)) event.preventDefault();
        return;
      }
      event.preventDefault();
      open();
    }
  };
  view.dom.addEventListener('keydown', key, true);
  return {
    open,
    destroy: () => {
      active?.abort.abort();
      view.dom.removeEventListener('keydown', key, true);
    },
  };
}
const inlineTheme = EditorView.baseTheme({
  '.cm-inline-review': {
    padding: '12px',
    whiteSpace: 'normal',
    border: '1px solid var(--or-separator)',
    background: 'var(--or-panel-solid)',
    color: 'var(--or-text)',
  },
  '.cm-inline-review pre': {
    whiteSpace: 'pre-wrap',
    maxHeight: '240px',
    overflow: 'auto',
  },
  '.cm-inline-review input': {
    width: 'min(480px, 100%)',
    color: 'inherit',
    background: 'var(--or-control-fill)',
    border: '1px solid var(--or-control-stroke)',
  },
  '.cm-inline-review button': {
    margin: '4px',
    color: 'inherit',
    background: 'var(--or-control-fill)',
  },
  '.cm-inline-original': { textDecoration: 'line-through', opacity: '0.65' },
});
