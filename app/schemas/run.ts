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

const requestEditorFieldsSchema = v.object({
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
  vus: positiveIntegerSchema,
  duration_ms: positiveIntegerSchema,
  timeout_ms: v.pipe(
    positiveIntegerSchema,
    v.maxValue(3_600_000, 'Timeout cannot exceed one hour.'),
  ),
  pacing_ms: nonnegativeIntegerSchema,
  grace_ms: nonnegativeIntegerSchema,
  max_vus: positiveIntegerSchema,
  max_request_bytes: positiveIntegerSchema,
});

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
    v.check(({ vus, max_vus }) => vus <= max_vus, 'VU count exceeds the worker budget.'),
    ['vus'],
  ),
  v.transform(({ bodyKind, bodyValue, max_vus, max_request_bytes, ...config }): RunConfig => ({
    ...config,
    body:
      bodyKind === 'none'
        ? { kind: 'none' }
        : {
            kind: bodyKind,
            value: bodyValue,
          },
    budget: { max_vus, max_request_bytes },
  })),
);

export const confirmationSettingsSchema = v.object({
  updateIntervalMs: v.pipe(v.picklist(['500', '1000', '2000']), v.transform(Number)),
});
