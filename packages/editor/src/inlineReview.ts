// SPDX-License-Identifier: Apache-2.0
import { isolateHistory } from '@codemirror/commands';
import { StateEffect, StateField } from '@codemirror/state';
import { Decoration, EditorView, WidgetType } from '@codemirror/view';
import type { DecorationSet } from '@codemirror/view';
import type { InlineBlock, InlineTransport } from './inline';
import { addedLines, button, icon, paragraph } from './inlineDom';
export { inlineTheme } from './inlineDom';
const PROPOSAL_LIMIT = 16384;
const STREAM_LIMIT = 32768;
const replace = StateEffect.define<DecorationSet>();
export const reviewField = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update: (value, transaction) => {
    if (transaction.docChanged) return Decoration.none;
    for (const effect of transaction.effects)
      if (effect.is(replace)) return effect.value;
    return value;
  },
  provide: (field) => EditorView.decorations.from(field),
});
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
type Status = 'prompt' | 'blocked' | 'running' | 'ready' | 'failed';
/** One prompt → stream → accept/reject cycle over a fixed original range. */
export class Review {
  readonly abort = new AbortController();
  readonly original: string;
  private readonly document: string;
  private text = '';
  private proposal = '';
  private status: Status;
  private failure = '';
  private block: InlineBlock | null;
  constructor(
    private readonly view: EditorView,
    private readonly transport: InlineTransport,
    readonly range: { from: number; to: number },
  ) {
    this.original = view.state.sliceDoc(range.from, range.to);
    this.document = view.state.doc.toString();
    this.block = transport.blocked?.() ?? null;
    this.status = this.block ? 'blocked' : 'prompt';
  }
  show(): void {
    const { from, to } = this.range;
    const marks = [
      Decoration.widget({
        widget: new ReviewWidget(this),
        block: true,
        side: 1,
      }).range(to),
    ];
    if (this.status !== 'blocked' && from !== to)
      marks.unshift(
        Decoration.mark({
          class:
            this.status === 'prompt'
              ? 'cm-inline-target'
              : 'cm-inline-original',
        }).range(from, to),
      );
    this.view.dispatch({ effects: replace.of(Decoration.set(marks, true)) });
  }
  /** Reject, Escape or Cancel: stop native work and restore the untouched document view. */
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
      annotations: isolateHistory.of('full'),
      userEvent: 'input.inline-proposal',
    });
    this.view.focus();
  }
  private async submit(prompt: string): Promise<void> {
    if (!prompt.trim() || this.status !== 'prompt') return;
    this.block = this.transport.blocked?.() ?? null;
    this.status = this.block ? 'blocked' : 'running';
    this.show();
    if (this.block) return;
    this.view.focus();
    try {
      const proposal = await this.transport.generate({
        prompt,
        sql: this.original,
        signal: this.abort.signal,
        delta: (text) => {
          if (this.abort.signal.aborted) return;
          this.text = text.slice(0, STREAM_LIMIT);
          this.show();
        },
      });
      if (this.abort.signal.aborted) return;
      if (!proposal.trim() || proposal.length > PROPOSAL_LIMIT)
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
      if (event.key !== 'Escape') return;
      event.preventDefault();
      event.stopPropagation();
      this.cancel();
    });
    if (this.status === 'prompt') this.prompt(host);
    else if (this.status === 'blocked') this.blocked(host);
    else this.result(host);
    return host;
  }
  /** `AI · Edit selection`: what the proposal will replace. */
  private heading(): HTMLElement {
    const heading = document.createElement('strong');
    heading.textContent = `AI · ${this.scope()}`;
    return heading;
  }
  private scope(): string {
    if (this.range.from === this.range.to) return 'Insert at cursor';
    const { from, to } = this.view.state.selection.main;
    return from === this.range.from && to === this.range.to
      ? 'Edit selection'
      : 'Edit current statement';
  }
  private blocked(host: HTMLElement): void {
    const block = this.block;
    const bar = document.createElement('div');
    bar.className = 'cm-inline-bar';
    bar.append(this.heading(), paragraph('alert', block?.reason ?? ''));
    const action = block?.action;
    if (action)
      bar.append(
        button(action.label, () => {
          this.cancel();
          action.run();
        }),
      );
    bar.append(
      button('Close', () => {
        this.cancel();
      }),
    );
    host.append(bar);
  }
  private result(host: HTMLElement): void {
    const bar = document.createElement('div');
    bar.className = 'cm-inline-bar';
    bar.append(
      this.heading(),
      paragraph(
        this.status === 'failed' ? 'alert' : 'status',
        this.failure ||
          (this.status === 'running'
            ? 'Generating SQL proposal… Escape cancels.'
            : 'Proposed SQL — review before running. Nothing runs automatically.'),
      ),
      button(
        this.status === 'running' ? 'Cancel proposal' : 'Reject proposal',
        () => {
          this.cancel();
        },
      ),
    );
    if (this.status === 'ready')
      bar.append(
        button(
          'Accept proposal',
          () => {
            this.accept();
          },
          'prominent',
        ),
      );
    host.append(bar, addedLines(this.proposal || this.text));
  }
  private prompt(host: HTMLElement): void {
    const form = document.createElement('form');
    form.className = 'cm-inline-prompt';
    const input = document.createElement('input');
    input.maxLength = 3500;
    input.placeholder =
      'Describe the SQL you want — Enter generates, Esc closes';
    input.setAttribute('aria-label', 'Inline SQL prompt');
    const scope = document.createElement('span');
    scope.className = 'cm-inline-tag';
    scope.textContent = `AI · ${this.scope()}`;
    form.append(
      icon('sparkles'),
      input,
      scope,
      button('Generate proposal', () => void this.submit(input.value)),
      button(
        'Close',
        () => {
          this.cancel();
        },
        'close',
      ),
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
