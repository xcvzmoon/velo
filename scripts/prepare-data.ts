import { open } from 'node:fs/promises';
import process from 'node:process';
import * as v from 'valibot';
import { prepareData, parseDataDefinition } from '../app/utils/dataset.ts';

async function readInput(path: string): Promise<string> {
  const file = await open(path, 'r');
  try {
    const buffer = Buffer.alloc(8388609);
    let length = 0;
    while (length < buffer.length) {
      // eslint-disable-next-line no-await-in-loop -- A bounded file read may return fewer bytes than requested.
      const { bytesRead } = await file.read(buffer, length, buffer.length - length, null);
      if (bytesRead === 0) {
        break;
      }
      length += bytesRead;
    }
    if (length > 8388608) {
      throw new Error('INPUT_TOO_LARGE');
    }
    return new TextDecoder('utf-8', { fatal: true }).decode(buffer.subarray(0, length));
  } finally {
    await file.close();
  }
}

async function main(): Promise<void> {
  const [definitionPath, sourcePath, ...extra] = process.argv.slice(2);
  if (!definitionPath || extra.length > 0) {
    throw new Error('INVALID_ARGUMENTS');
  }
  const definitionSource = await readInput(definitionPath);
  const definition = v.parse(v.object({ dataset: v.unknown() }), JSON.parse(definitionSource));
  const settings = parseDataDefinition(JSON.stringify(definition.dataset));
  if ((settings.source.kind === 'faker') === Boolean(sourcePath)) {
    throw new Error('DATASET_SOURCE_REQUIRED');
  }
  const source = sourcePath ? await readInput(sourcePath) : '';
  const rows = await prepareData(settings, source, new AbortController().signal);
  process.stdout.write(JSON.stringify(rows));
}

main().catch(() => {
  process.stderr.write(
    'DATASET_PREPARATION_FAILED: Check the definition, source file, and dataset limits.\n',
  );
  process.exitCode = 2;
});
