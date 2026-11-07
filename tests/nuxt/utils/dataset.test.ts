import type { DataDefinition } from '../../../app/types/contracts';
import { describe, expect, it } from 'vitest';
import { parseDataDefinition, prepareData } from '../../../app/utils/dataset';

function recipe(): DataDefinition {
  return {
    lifetime: 'iteration',
    allocation: 'unique',
    exhaustion: 'stop',
    seed: 42,
    source: {
      kind: 'faker',
      version: '10.6.0',
      locale: 'en',
      count: 5,
      fields: [
        {
          name: 'first',
          generator: { kind: 'first_name' },
        },
        {
          name: 'last',
          generator: { kind: 'last_name' },
        },
        {
          name: 'email',
          generator: {
            kind: 'email',
            first: 'first',
            last: 'last',
            provider: 'example.test',
          },
        },
        {
          name: 'copy',
          generator: {
            kind: 'copy',
            field: 'email',
          },
        },
        {
          name: 'invalid',
          generator: {
            kind: 'literal',
            value: '',
          },
        },
        {
          name: 'boundary',
          generator: {
            kind: 'integer',
            min: -1,
            max: -1,
          },
        },
      ],
    },
  };
}
function prepare(definition: DataDefinition, source = ''): Promise<Record<string, string>[]> {
  return prepareData(definition, source, new AbortController().signal);
}
describe('dataset preparation', () => {
  it('reproduces pinned seeded Faker output and related fields', async () => {
    const definition = recipe();
    const first = await prepare(definition);
    expect(await prepare(definition)).toEqual(first);
    expect(first[0]).toMatchObject({
      first: 'Nikita',
      last: 'Crist',
      invalid: '',
      boundary: '-1',
    });
    for (const row of first) {
      expect(row.copy).toBe(row.email);
      expect(row.email).toContain('@example.test');
    }
    definition.seed = 43;
    expect(await prepare(definition)).not.toEqual(first);
  });
  it('supports quoted CSV, Unicode, JSON scalars, and explicit mappings', async () => {
    const definition = recipe();
    definition.source = {
      kind: 'csv',
      mapping: {
        name: 'full name',
        id: 'id',
      },
    };
    expect(await prepare(definition, 'id,full name\r\n1,"Alice, é"\r\n2,"Bob\nSmith"')).toEqual([
      {
        id: '1',
        name: 'Alice, é',
      },
      {
        id: '2',
        name: 'Bob\nSmith',
      },
    ]);
    definition.source = {
      kind: 'json',
      mapping: {
        name: 'label',
        id: 'id',
      },
    };
    expect(await prepare(definition, '[{"id":1,"label":false}]')).toEqual([
      {
        id: '1',
        name: 'false',
      },
    ]);
  });
  it('reports malformed imports, missing columns, and unsupported values', async () => {
    const definition = recipe();
    definition.source = {
      kind: 'csv',
      mapping: { id: 'id' },
    };
    await expect(prepare(definition, 'id,id\n1,2')).rejects.toThrow('header names');
    await expect(prepare(definition, 'id,name\n1')).rejects.toThrow('row 2');
    await expect(prepare(definition, 'name\nAlice')).rejects.toThrow('column id');
    definition.source = {
      kind: 'json',
      mapping: { id: 'id' },
    };
    for (const input of [
      '{}',
      '[{"id":null}]',
      '[{"id":{}}]',
      '[{"id":[]}]',
      '[{"id":1e999}]',
      '[]',
    ]) {
      // eslint-disable-next-line no-await-in-loop -- Each input is an independent malformed import case.
      await expect(prepare(definition, input)).rejects.toThrow('DATASET_IMPORT_FAILED');
    }
  });
  it('stops oversized CSV imports before mapping records', async () => {
    const definition = recipe();
    definition.source = {
      kind: 'csv',
      mapping: { id: 'id' },
    };
    await expect(prepare(definition, `id\n${'1\n'.repeat(10001)}`)).rejects.toThrow(
      'DATASET_BUDGET_EXCEEDED',
    );
  });

  it('rejects scripts, unsupported arguments, versions, cycles, and repeat uniqueness', () => {
    const definition = recipe();
    const value = JSON.stringify(definition);
    expect(() => parseDataDefinition(value.replace('first_name', 'eval'))).toThrow(
      'INVALID_DATASET',
    );
    expect(() => parseDataDefinition(value.replace('10.6.0', '9.0.0'))).toThrow('INVALID_DATASET');
    expect(() =>
      parseDataDefinition(
        value.replace('"kind":"first_name"', '"kind":"first_name","script":"alert(1)"'),
      ),
    ).toThrow('INVALID_DATASET');
    definition.exhaustion = 'repeat';
    expect(() => parseDataDefinition(JSON.stringify(definition))).toThrow('Unique allocation');
  });
  it('cancels preparation and enforces encoded byte and record limits', async () => {
    const definition = recipe();
    const controller = new AbortController();
    const promise = prepareData(definition, '', controller.signal);
    controller.abort();
    await expect(promise).rejects.toThrow('PREPARATION_CANCELLED');
    if (definition.source.kind === 'faker') {
      definition.source.count = 10001;
      await expect(prepare(definition)).rejects.toThrow('INVALID_DATASET');
      definition.source.count = 1000;
      definition.source.fields = [
        {
          name: 'value',
          generator: {
            kind: 'literal',
            value: 'é'.repeat(8192),
          },
        },
      ];
      await expect(prepare(definition)).rejects.toThrow('DATASET_BUDGET_EXCEEDED');
    }
  });
});
