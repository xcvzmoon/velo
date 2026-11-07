import type { DataDefinition, DataField, DataGenerator } from '../types/contracts';
import { Faker } from '@faker-js/faker';
import Papa from 'papaparse';
import * as v from 'valibot';

const nameSchema = v.pipe(v.string(), v.regex(/^[A-Za-z0-9_.-]{1,80}$/));
const referenceSchema = nameSchema;
const generatorSchema = v.variant('kind', [
  v.strictObject({ kind: v.literal('first_name') }),
  v.strictObject({ kind: v.literal('last_name') }),
  v.strictObject({ kind: v.literal('uuid') }),
  v.strictObject({
    kind: v.literal('email'),
    first: referenceSchema,
    last: referenceSchema,
    provider: v.pipe(v.string(), v.minLength(1), v.maxLength(253), v.regex(/^[A-Za-z0-9.-]+$/)),
  }),
  v.pipe(
    v.strictObject({
      kind: v.literal('integer'),
      min: v.pipe(v.number(), v.integer(), v.minValue(-2147483648), v.maxValue(2147483647)),
      max: v.pipe(v.number(), v.integer(), v.minValue(-2147483648), v.maxValue(2147483647)),
    }),
    v.check(({ min, max }) => min <= max, 'Minimum must not exceed maximum.'),
  ),
  v.strictObject({
    kind: v.literal('literal'),
    value: v.pipe(
      v.string(),
      v.check(
        (value) => new TextEncoder().encode(value).length <= 16384,
        'Literal values must not exceed 16 KiB.',
      ),
    ),
  }),
  v.strictObject({
    kind: v.literal('copy'),
    field: referenceSchema,
  }),
]);
const fieldsSchema = v.pipe(
  v.array(
    v.strictObject({
      name: nameSchema,
      generator: generatorSchema,
    }),
  ),
  v.minLength(1),
  v.maxLength(64),
  v.check(validFields, 'Use unique names; related fields must refer to earlier fields.'),
);
const mappingSchema = v.pipe(
  v.record(nameSchema, v.pipe(v.string(), v.minLength(1), v.maxLength(256))),
  v.check(
    (mapping) => Object.keys(mapping).length > 0 && Object.keys(mapping).length <= 64,
    'Map 1 to 64 columns.',
  ),
);
const sourceSchema = v.variant('kind', [
  v.strictObject({
    kind: v.literal('faker'),
    version: v.literal('10.6.0'),
    locale: v.picklist(['en', 'de', 'fr']),
    count: v.pipe(v.number(), v.integer(), v.minValue(1), v.maxValue(10000)),
    fields: fieldsSchema,
  }),
  v.strictObject({
    kind: v.literal('csv'),
    mapping: mappingSchema,
  }),
  v.strictObject({
    kind: v.literal('json'),
    mapping: mappingSchema,
  }),
]);
const definitionSchema = v.pipe(
  v.strictObject({
    lifetime: v.picklist(['run', 'user', 'iteration']),
    allocation: v.picklist(['sequential', 'random', 'unique']),
    exhaustion: v.picklist(['stop', 'repeat', 'fail']),
    seed: v.pipe(v.number(), v.integer(), v.minValue(0), v.maxValue(4294967295)),
    source: sourceSchema,
  }),
  v.check(
    ({ allocation, exhaustion }) => allocation !== 'unique' || exhaustion !== 'repeat',
    'Unique allocation cannot repeat records.',
  ),
);
const rowsSchema = v.pipe(
  v.array(v.record(v.string(), v.union([v.string(), v.pipe(v.number(), v.finite()), v.boolean()]))),
  v.minLength(1),
  v.maxLength(10000),
);
function validFields(fields: DataField[]): boolean {
  const names = new Set<string>();
  for (const { name, generator } of fields) {
    if (names.has(name)) {
      return false;
    }
    if (generator.kind === 'email' && (!names.has(generator.first) || !names.has(generator.last))) {
      return false;
    }
    if (generator.kind === 'copy' && !names.has(generator.field)) {
      return false;
    }
    names.add(name);
  }
  return true;
}
function generatedValue(
  faker: Faker,
  generator: DataGenerator,
  row: ReadonlyMap<string, string>,
): string {
  switch (generator.kind) {
    case 'first_name':
      return faker.person.firstName();
    case 'last_name':
      return faker.person.lastName();
    case 'uuid':
      return faker.string.uuid();
    case 'email':
      return faker.internet.email({
        firstName: row.get(generator.first),
        lastName: row.get(generator.last),
        provider: generator.provider,
      });
    case 'integer':
      return String(
        faker.number.int({
          min: generator.min,
          max: generator.max,
        }),
      );
    case 'literal':
      return generator.value;
    case 'copy':
      return row.get(generator.field) ?? '';
  }
  return generator satisfies never;
}
/**
 * Parses a portable dataset recipe without evaluating scripts or preparing records.
 * @param source - JSON settings, limited to 64 KiB.
 * @returns Validated settings with supported generators and references.
 * @throws Error with INVALID_DATASET when settings are malformed or unsupported.
 * @example
 * ```ts
 * const settings = parseDataDefinition('{"lifetime":"iteration","allocation":"unique","exhaustion":"stop","seed":42,"source":{"kind":"json","mapping":{"id":"id"}}}')
 * ```
 */
export function parseDataDefinition(source: string): DataDefinition {
  if (new TextEncoder().encode(source).length > 65536) {
    throw new Error('INVALID_DATASET: Settings exceed 64 KiB.');
  }
  let input: Parameters<typeof v.safeParse>[1];
  try {
    input = JSON.parse(source);
  } catch {
    throw new Error('INVALID_DATASET: Enter valid JSON settings.');
  }
  const parsed = v.safeParse(definitionSchema, input);
  if (!parsed.success) {
    const issue = parsed.issues[0];
    const path = issue?.path?.map(({ key }) => String(key)).join('.') ?? 'dataset';
    throw new Error(`INVALID_DATASET (${path}): ${issue?.message ?? 'Check settings.'}`);
  }
  return parsed.output;
}
/**
 * Prepares all dataset rows before native review, yielding between bounded batches.
 * @param definition - Dataset settings, validated again at this boundary.
 * @param source - Imported UTF-8 CSV/JSON content, empty for Faker.
 * @param signal - Cancellation signal; cancellation discards partial results.
 * @returns At most 10,000 scalar records and 8 MiB of encoded data, held only in memory.
 * @throws Error with INVALID_DATASET, DATASET_IMPORT_FAILED, DATASET_BUDGET_EXCEEDED, or PREPARATION_CANCELLED.
 * @example
 * ```ts
 * const rows = await prepareData(settings, fileText, new AbortController().signal)
 * ```
 */
export async function prepareData(
  definition: DataDefinition,
  source: string,
  signal: AbortSignal,
): Promise<Record<string, string>[]> {
  const settings = parseDataDefinition(JSON.stringify(definition));
  const encoder = new TextEncoder();
  if (encoder.encode(source).length > 8388608) {
    throw new Error('DATASET_BUDGET_EXCEEDED: Import exceeds 8 MiB.');
  }
  const rows: Record<string, string>[] = [];
  let bytes = 0;
  function append(row: Record<string, string>): void {
    bytes += encoder.encode(JSON.stringify(row)).length;
    if (rows.length >= 10000 || bytes > 8388608) {
      throw new Error('DATASET_BUDGET_EXCEEDED: Use at most 10,000 rows and 8 MiB.');
    }
    rows.push(row);
  }
  async function checkpoint(index: number): Promise<void> {
    if (index % 64 === 0) {
      await new Promise<void>((resolve) => setTimeout(resolve, 0));
    }
    if (signal.aborted) {
      throw new Error('PREPARATION_CANCELLED: Prepared data was discarded.');
    }
  }
  if (settings.source.kind === 'faker') {
    const { locale, count, fields } = settings.source;
    const localized =
      locale === 'de'
        ? await import('@faker-js/faker/locale/de')
        : locale === 'fr'
          ? await import('@faker-js/faker/locale/fr')
          : await import('@faker-js/faker/locale/en');
    const faker = new Faker({ locale: localized.faker.rawDefinitions });
    faker.seed(settings.seed);
    for (let index = 0; index < count; index++) {
      // eslint-disable-next-line no-await-in-loop -- Sequential batches preserve seeded order and permit cancellation.
      await checkpoint(index);
      const row = new Map<string, string>();
      for (const { name, generator } of fields) {
        row.set(name, generatedValue(faker, generator, row));
      }
      append(Object.fromEntries(row));
    }
  } else {
    let input: Parameters<typeof v.safeParse>[1];
    if (settings.source.kind === 'csv') {
      const header =
        Papa.parse<string[]>(source, {
          preview: 1,
          delimiter: ',',
        }).data[0] ?? [];
      if (new Set(header).size !== header.length || header.some((name) => name.length === 0)) {
        throw new Error('DATASET_IMPORT_FAILED: CSV header names must be nonempty and unique.');
      }
      const parsed = Papa.parse<Record<string, string>>(source, {
        header: true,
        preview: 10001,
        delimiter: ',',
        skipEmptyLines: 'greedy',
        dynamicTyping: false,
      });
      const error = parsed.errors[0];
      if (error) {
        throw new Error(
          `DATASET_IMPORT_FAILED (row ${(error.row ?? 0) + 2}): ${error.code}. Check column count and quoting.`,
        );
      }
      input = parsed.data;
    } else {
      try {
        input = JSON.parse(source);
      } catch {
        throw new Error('DATASET_IMPORT_FAILED: Enter a JSON array of records.');
      }
    }
    if (Array.isArray(input) && input.length > 10000) {
      throw new Error('DATASET_BUDGET_EXCEEDED: Use at most 10,000 records.');
    }
    const parsed = v.safeParse(rowsSchema, input, { abortEarly: true });
    if (!parsed.success) {
      const path = parsed.issues[0]?.path?.map(({ key }) => String(key)).join('.') ?? 'rows';
      throw new Error(
        `DATASET_IMPORT_FAILED (${path}): Use 1 to 10,000 records containing strings, finite numbers, or booleans. Nulls and nested values are unsupported.`,
      );
    }
    for (const [index, entry] of parsed.output.entries()) {
      // eslint-disable-next-line no-await-in-loop -- Sequential batches preserve seeded order and permit cancellation.
      await checkpoint(index);
      const row = new Map<string, string>();
      for (const [name, column] of Object.entries(settings.source.mapping)) {
        if (!Object.hasOwn(entry, column)) {
          throw new Error(
            `DATASET_IMPORT_FAILED (row ${index + 1}, column ${column}): Mapped field is missing.`,
          );
        }
        row.set(name, String(entry[column]));
      }
      append(Object.fromEntries(row));
    }
  }
  if (signal.aborted) {
    throw new Error('PREPARATION_CANCELLED: Prepared data was discarded.');
  }
  return rows;
}
