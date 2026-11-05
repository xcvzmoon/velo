import type { RunConfig } from '../types/contracts';
import * as v from 'valibot';

const positiveIntegerSchema = v.pipe(
  v.union([v.string(), v.number()]),
  v.toNumber(),
  v.integer('Enter a whole number.'),
  v.minValue(1, 'Enter a positive value.'),
  v.maxValue(4_294_967_295, 'The value exceeds the supported representation.'),
);

const nonnegativeIntegerSchema = v.pipe(
  v.union([v.string(), v.number()]),
  v.toNumber(),
  v.integer('Enter a whole number.'),
  v.minValue(0, 'Enter zero or a positive value.'),
  v.maxValue(4_294_967_295, 'The value exceeds the supported representation.'),
);

const requestFieldsSchema = v.object({
  url: v.pipe(
    v.string(),
    v.url('Enter an absolute HTTP(S) URL.'),
    v.check((value) => {
      try {
        const url = new URL(value);
        return (
          ['http:', 'https:'].includes(url.protocol) &&
          url.hostname.length > 0 &&
          !url.username &&
          !url.password &&
          !url.hash &&
          !value.includes('#')
        );
      } catch {
        return false;
      }
    }, 'Use HTTP(S) without URL credentials or a fragment.'),
  ),
  method: v.picklist(['GET', 'POST']),
  headers: v.array(
    v.object({
      name: v.pipe(
        v.string(),
        v.regex(/^[!#$%&'*+.^_`|~\w-]+$/, 'Enter a valid HTTP header name.'),
        v.check(
          (name) => !['host', 'content-length', 'transfer-encoding'].includes(name.toLowerCase()),
          'Host and body framing headers are managed by the client.',
        ),
      ),
      value: v.pipe(
        v.string(),
        v.regex(/^[\t\x20-\x7E\u0080-\uFFFF]*$/, 'Enter a valid header value.'),
      ),
    }),
  ),
  query: v.array(
    v.object({
      name: v.string(),
      value: v.string(),
    }),
  ),
  bodyKind: v.picklist(['none', 'text', 'json']),
  bodyValue: v.string(),
  timeout_ms: v.pipe(
    positiveIntegerSchema,
    v.maxValue(3_600_000, 'Timeout cannot exceed one hour.'),
  ),
  pacing_ms: nonnegativeIntegerSchema,
  grace_ms: nonnegativeIntegerSchema,
  max_vus: positiveIntegerSchema,
  max_request_bytes: positiveIntegerSchema,
});

const numericInputSchema = v.union([v.string(), v.number()]);
const inactiveStagesSchema = v.array(
  v.object({
    duration_ms: numericInputSchema,
    target_vus: numericInputSchema,
  }),
);

const requestEditorFieldsSchema = v.variant('workloadKind', [
  v.object({
    ...requestFieldsSchema.entries,
    workloadKind: v.literal('constant'),
    vus: positiveIntegerSchema,
    duration_ms: positiveIntegerSchema,
    start_vus: numericInputSchema,
    stages: inactiveStagesSchema,
  }),
  v.object({
    ...requestFieldsSchema.entries,
    workloadKind: v.literal('ramping'),
    vus: numericInputSchema,
    duration_ms: numericInputSchema,
    start_vus: nonnegativeIntegerSchema,
    stages: v.array(
      v.object({
        duration_ms: positiveIntegerSchema,
        target_vus: nonnegativeIntegerSchema,
      }),
    ),
  }),
]);

export const requestEditorSchema = v.pipe(
  requestEditorFieldsSchema,
  v.forward(
    v.check(
      ({ method, bodyKind }) => method !== 'GET' || bodyKind === 'none',
      'GET requests cannot contain a body.',
    ),
    ['bodyKind'],
  ),
  v.forward(
    v.check(
      ({ bodyKind, bodyValue }) =>
        bodyKind !== 'json' || v.is(v.pipe(v.string(), v.parseJson()), bodyValue),
      'Enter a valid JSON body.',
    ),
    ['bodyValue'],
  ),
  v.forward(
    v.check(
      ({ workloadKind, vus, start_vus, stages, max_vus }) =>
        workloadKind === 'constant'
          ? vus <= max_vus
          : start_vus <= max_vus && stages.every(({ target_vus }) => target_vus <= max_vus),
      'Peak VUs exceed the worker budget.',
    ),
    ['max_vus'],
  ),
  v.forward(
    v.check(({ workloadKind, stages }) => {
      if (workloadKind === 'constant') {
        return true;
      }

      if (stages.length === 0) {
        return false;
      }

      let total = 0;

      for (const { duration_ms } of stages) {
        total += duration_ms;
      }

      return total <= 4_294_967_295;
    }, 'Add at least one stage; total duration must not exceed 4,294,967,295 ms.'),
    ['stages'],
  ),
  v.transform(
    ({
      bodyKind,
      bodyValue,
      max_vus,
      max_request_bytes,
      workloadKind,
      vus,
      duration_ms,
      start_vus,
      stages,
      ...config
    }): RunConfig => ({
      ...config,
      workload:
        workloadKind === 'constant'
          ? {
              kind: 'constant',
              vus,
              duration_ms,
            }
          : {
              kind: 'ramping',
              start_vus,
              stages,
            },
      body:
        bodyKind === 'none'
          ? { kind: 'none' }
          : {
              kind: bodyKind,
              value: bodyValue,
            },
      budget: { max_vus, max_request_bytes },
    }),
  ),
);

export const confirmationSettingsSchema = v.object({
  updateIntervalMs: v.pipe(v.picklist(['500', '1000', '2000']), v.transform(Number)),
});
