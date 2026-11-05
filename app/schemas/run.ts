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

const thresholdSchema = v.pipe(
  v.object({
    metric: v.picklist(['response_p95_ms', 'http_failure_percent', 'request_rate']),
    operator: v.picklist(['less_than', 'greater_than']),
    limit: v.pipe(
      v.union([v.pipe(v.string(), v.nonEmpty('Enter a limit.')), v.number()]),
      v.toNumber(),
      v.finite('Enter a finite limit.'),
      v.minValue(0, 'Enter zero or a positive limit.'),
    ),
  }),
  v.forward(
    v.check(
      ({ metric, limit }) =>
        metric === 'http_failure_percent'
          ? limit <= 100
          : metric === 'response_p95_ms'
            ? limit <= 3_600_000
            : true,
      'Percentages cannot exceed 100; latency cannot exceed one hour.',
    ),
    ['limit'],
  ),
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
  method: v.picklist(['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS']),
  authKind: v.picklist(['none', 'bearer', 'basic']),
  authToken: v.string(),
  authUsername: v.string(),
  authPassword: v.string(),
  protocol: v.picklist(['auto', 'http1', 'http2']),
  max_redirects: nonnegativeIntegerSchema,
  thresholds: v.array(thresholdSchema),
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
  bodyKind: v.picklist(['none', 'text', 'json', 'form', 'multipart']),
  formPairs: v.array(
    v.object({
      name: v.string(),
      value: v.string(),
    }),
  ),
  multipartParts: v.array(
    v.object({
      kind: v.picklist(['text', 'file']),
      name: v.string(),
      value: v.string(),
      path: v.string(),
      filename: v.string(),
      content_type: v.string(),
    }),
  ),
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
    workloadKind: v.literal('arrival'),
    rate: positiveIntegerSchema,
    capacity: positiveIntegerSchema,
    vus: numericInputSchema,
    duration_ms: positiveIntegerSchema,
    start_vus: numericInputSchema,
    stages: inactiveStagesSchema,
    pacing_ms: v.pipe(
      nonnegativeIntegerSchema,
      v.maxValue(0, 'Arrival workloads require zero pacing.'),
    ),
  }),
  v.object({
    ...requestFieldsSchema.entries,
    workloadKind: v.literal('constant'),
    rate: numericInputSchema,
    capacity: numericInputSchema,
    vus: positiveIntegerSchema,
    duration_ms: positiveIntegerSchema,
    start_vus: numericInputSchema,
    stages: inactiveStagesSchema,
  }),
  v.object({
    ...requestFieldsSchema.entries,
    workloadKind: v.literal('ramping'),
    rate: numericInputSchema,
    capacity: numericInputSchema,
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
      ({ method, bodyKind }) => !['GET', 'HEAD'].includes(method) || bodyKind === 'none',
      'GET and HEAD requests cannot contain a body.',
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
      ({ workloadKind, vus, start_vus, stages, max_vus, capacity }) =>
        workloadKind === 'arrival'
          ? capacity <= max_vus
          : workloadKind === 'constant'
            ? vus <= max_vus
            : start_vus <= max_vus && stages.every(({ target_vus }) => target_vus <= max_vus),
      'Peak VUs exceed the worker budget.',
    ),
    ['max_vus'],
  ),
  v.forward(
    v.check(({ workloadKind, stages }) => {
      if (workloadKind !== 'ramping') {
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
  v.forward(
    v.check(({ authKind, authToken, authUsername, authPassword, headers }) => {
      if (authKind === 'none') {
        return true;
      }

      if (headers.some(({ name }) => name.toLowerCase() === 'authorization')) {
        return false;
      }

      return authKind === 'bearer'
        ? /^[A-Za-z0-9._~+/-]+=*$/.test(authToken)
        : !authUsername.includes(':') && !/\p{Cc}/u.test(authUsername + authPassword);
    }, 'Enter valid credentials and remove conflicting Authorization headers.'),
    ['authKind'],
  ),
  v.forward(
    v.check(
      ({ bodyKind, headers, multipartParts }) =>
        bodyKind !== 'multipart' ||
        (!headers.some(({ name }) => name.toLowerCase() === 'content-type') &&
          multipartParts.every(
            (part) =>
              part.name.length > 0 &&
              !/\p{Cc}/u.test(part.name) &&
              (part.kind === 'text' ||
                (part.path.length > 0 &&
                  part.filename.length > 0 &&
                  !/[\\/\p{Cc}]/u.test(part.filename) &&
                  /^[\w!#$&^.+-]+\/[\w!#$&^.+-]+$/.test(part.content_type))),
          )),
      'Multipart needs valid names, selected files, filenames and MIME types; remove custom Content-Type headers.',
    ),
    ['bodyKind'],
  ),
  v.transform(
    ({
      bodyKind,
      bodyValue,
      formPairs,
      multipartParts,
      authKind,
      authToken,
      authUsername,
      authPassword,
      max_vus,
      max_request_bytes,
      workloadKind,
      rate,
      capacity,
      vus,
      duration_ms,
      start_vus,
      stages,
      ...config
    }): RunConfig => ({
      ...config,
      auth:
        authKind === 'none'
          ? { kind: 'none' }
          : authKind === 'bearer'
            ? {
                kind: 'bearer',
                token: authToken,
              }
            : {
                kind: 'basic',
                username: authUsername,
                password: authPassword,
              },
      workload:
        workloadKind === 'arrival'
          ? {
              kind: 'arrival',
              rate,
              capacity,
              duration_ms,
            }
          : workloadKind === 'constant'
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
          : bodyKind === 'form'
            ? {
                kind: 'form',
                value: formPairs,
              }
            : bodyKind === 'multipart'
              ? {
                  kind: 'multipart',
                  value: multipartParts.map((part) =>
                    part.kind === 'text'
                      ? {
                          kind: 'text',
                          name: part.name,
                          value: part.value,
                        }
                      : {
                          kind: 'file',
                          name: part.name,
                          path: part.path,
                          filename: part.filename,
                          content_type: part.content_type,
                        },
                  ),
                }
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
