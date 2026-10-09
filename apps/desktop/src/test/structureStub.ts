// SPDX-License-Identifier: Apache-2.0
import { vi } from 'vitest';
import type {
  AlterPreviewDto,
  AlterReply,
  ObjectReply,
  TableDetailDto,
} from '@openrows/ipc';

/** A synthetic `orders` table: `id` (primary key), `name`, `customer_id` (FK). */
export const sampleDetail: TableDetailDto = {
  columns: [
    {
      name: 'id',
      dataType: 'integer',
      nullable: false,
      default: null,
      generated: 'identityAlways',
      comment: null,
      check: null,
      foreignKey: null,
    },
    {
      name: 'name',
      dataType: 'text',
      nullable: true,
      default: "'anon'::text",
      generated: 'no',
      comment: 'Display name',
      check: '(length(name) > 0)',
      foreignKey: null,
    },
    {
      name: 'customer_id',
      dataType: 'uuid',
      nullable: true,
      default: null,
      generated: 'no',
      comment: null,
      check: null,
      foreignKey: { schema: 'public', table: 'customers', column: 'id' },
    },
  ],
  primaryKey: ['id'],
  indexes: [
    {
      name: 'orders_pkey',
      method: 'btree',
      unique: true,
      primary: true,
      constraint: true,
      columns: 'id',
      include: [],
      predicate: null,
      comment: null,
      definition:
        'CREATE UNIQUE INDEX orders_pkey ON public.orders USING btree (id)',
    },
    {
      name: 'orders_name_idx',
      method: 'btree',
      unique: false,
      primary: false,
      constraint: false,
      columns: 'lower(name)',
      include: [],
      predicate: 'name IS NOT NULL',
      comment: null,
      definition:
        'CREATE INDEX orders_name_idx ON public.orders USING btree (lower(name)) WHERE name IS NOT NULL',
    },
  ],
  partitions: [],
  triggers: [
    {
      name: 'orders_touch',
      timing: 'before',
      events: ['UPDATE'],
      rowLevel: true,
      function: 'public.touch()',
      enabled: true,
      definition:
        'CREATE TRIGGER orders_touch BEFORE UPDATE ON public.orders FOR EACH ROW EXECUTE FUNCTION touch()',
    },
  ],
  info: {
    kind: 'table',
    rowEstimate: 1788,
    totalBytes: 82944,
    dataBytes: 16384,
    indexBytes: 66560,
    comment: 'Customer orders',
  },
  constraints: [
    {
      name: 'orders_pkey',
      kind: 'primary',
      definition: 'PRIMARY KEY (id)',
      columns: ['id'],
      references: null,
    },
    {
      name: 'orders_customer_id_fkey',
      kind: 'foreign',
      definition:
        'FOREIGN KEY (customer_id) REFERENCES public.customers(id) ON DELETE CASCADE',
      columns: ['customer_id'],
      references: { schema: 'public', table: 'customers', columns: ['id'] },
    },
    {
      name: 'orders_name_check',
      kind: 'check',
      definition: 'CHECK ((length(name) > 0))',
      columns: ['name'],
      references: null,
    },
  ],
  viewQuery: null,
  ddl: '-- Table Definition\nCREATE TABLE "public"."orders" (\n    "id" integer GENERATED ALWAYS AS IDENTITY\n);\n',
  editable: true,
};

/** A preview of staged changes: one statement per change, no warnings. */
function preview(
  _target: unknown,
  changes: readonly unknown[],
): Promise<AlterPreviewDto> {
  return Promise.resolve({
    statements: changes.map((change) => `-- ${JSON.stringify(change)}`),
    warnings: [],
    destructive: false,
  });
}

/** Structure IPC for jsdom tests: detail reads, previews and prepare-only changes. */
export function structureStub(): Record<string, unknown> {
  return {
    tableDetail: vi.fn(() => Promise.resolve(sampleDetail)),
    tableAlterPreview: vi.fn(preview),
    objectChange: vi.fn(
      (request: { confirmToken: string | null }): Promise<ObjectReply> =>
        Promise.resolve(
          request.confirmToken
            ? { kind: 'committed' }
            : {
                kind: 'prepared',
                token: 'object-review',
                statements: ['DROP TABLE "public"."orders"'],
                warnings: [
                  'The table and every row in it are deleted permanently.',
                ],
                destructive: true,
                confirmName: null,
                transactional: true,
              },
        ),
    ),
    routineDefinition: vi.fn(() =>
      Promise.resolve(
        'CREATE OR REPLACE FUNCTION public.touch()\n RETURNS trigger\n LANGUAGE plpgsql\nAS $function$ BEGIN RETURN NEW; END $function$\n',
      ),
    ),
    tableAlter: vi.fn((): Promise<AlterReply> =>
      Promise.resolve({
        kind: 'prepared',
        token: 'review',
        statements: [],
        warnings: [],
        destructive: false,
        confirmName: null,
      }),
    ),
  };
}
