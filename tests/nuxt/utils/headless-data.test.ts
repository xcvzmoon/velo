import type { DataDefinition } from '../../../app/types/contracts';
import { execFile } from 'node:child_process';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import process from 'node:process';
import { promisify } from 'node:util';
import * as v from 'valibot';
import { describe, expect, it } from 'vitest';
import { prepareData } from '../../../app/utils/dataset';

const execute = promisify(execFile);
const helper = resolve('scripts/prepare-data.ts');

async function headlessRows(
  settings: DataDefinition,
  source: string | null,
): Promise<Record<string, string>[]> {
  const directory = await mkdtemp(join(tmpdir(), 'velo-headless-data-'));
  try {
    const definition = join(directory, 'velo.test.json');
    await writeFile(definition, JSON.stringify({ dataset: settings }));
    const args = [helper, definition];
    if (source !== null) {
      const input = join(directory, 'source');
      await writeFile(input, source);
      args.push(input);
    }
    const { stdout } = await execute(process.execPath, args);
    return v.parse(v.array(v.record(v.string(), v.string())), JSON.parse(stdout));
  } finally {
    await rm(directory, {
      recursive: true,
      force: true,
    });
  }
}

describe('headless dataset preparation', () => {
  it('uses the desktop Faker version, locale, seed, and row order', async () => {
    const settings: DataDefinition = {
      seed: 42,
      lifetime: 'iteration',
      allocation: 'sequential',
      exhaustion: 'repeat',
      source: {
        kind: 'faker',
        version: '10.6.0',
        locale: 'fr',
        count: 4,
        fields: [
          {
            name: 'name',
            generator: { kind: 'first_name' },
          },
        ],
      },
    };
    const desktop = await prepareData(settings, '', new AbortController().signal);
    expect(await headlessRows(settings, null)).toEqual(desktop);
  });

  it.each(['csv', 'json'] as const)('uses desktop %s parsing and mappings', async (kind) => {
    const settings: DataDefinition = {
      seed: 42,
      lifetime: 'iteration',
      allocation: 'unique',
      exhaustion: 'stop',
      source: {
        kind,
        mapping: { item: 'id' },
      },
    };
    const source = kind === 'csv' ? 'id\n"first, item"\nsecond\n' : '[{"id":"first"},{"id":123}]';
    const desktop = await prepareData(settings, source, new AbortController().signal);
    expect(await headlessRows(settings, source)).toEqual(desktop);
  });

  it('rejects unsupported generators without echoing source values', async () => {
    const directory = await mkdtemp(join(tmpdir(), 'velo-headless-invalid-'));
    try {
      const definition = join(directory, 'velo.test.json');
      await writeFile(definition, '{"dataset":{"source":{"kind":"PRIVATE_INPUT_VALUE"}}}');
      await expect(execute(process.execPath, [helper, definition])).rejects.toMatchObject({
        code: 2,
        stdout: '',
        stderr:
          'DATASET_PREPARATION_FAILED: Check the definition, source file, and dataset limits.\n',
      });
    } finally {
      await rm(directory, {
        recursive: true,
        force: true,
      });
    }
  });
});
