import * as v from 'valibot';
import { describe, expect, it } from 'vitest';
import { requestEditorSchema } from '../../../app/schemas/run';

const input = {
  url: 'http://127.0.0.1:8080/items',
  method: 'POST',
  headers: [
    {
      name: 'X-Request-Label',
      value: 'local run',
    },
  ],
  query: [
    {
      name: 'tag',
      value: 'first',
    },
    {
      name: 'tag',
      value: 'second',
    },
  ],
  bodyKind: 'json',
  bodyValue: '{"enabled":true}',
  workloadKind: 'constant',
  vus: '2',
  start_vus: '0',
  stages: [],
  duration_ms: '1000',
  timeout_ms: '30000',
  pacing_ms: '0',
  grace_ms: '5000',
  max_vus: '256',
  max_request_bytes: '8388608',
};

describe('request editor schema', () => {
  it('converts form inputs to a native request without losing duplicate pairs or body bytes', () => {
    expect(v.parse(requestEditorSchema, input)).toEqual({
      url: input.url,
      method: 'POST',
      headers: input.headers,
      query: input.query,
      body: {
        kind: 'json',
        value: input.bodyValue,
      },
      workload: {
        kind: 'constant',
        vus: 2,
        duration_ms: 1000,
      },
      timeout_ms: 30_000,
      pacing_ms: 0,
      grace_ms: 5000,
      budget: {
        max_vus: 256,
        max_request_bytes: 8_388_608,
      },
    });
  });

  it('discards inactive body text and accepts zero pacing and immediate cancellation grace', () => {
    const output = v.parse(requestEditorSchema, {
      ...input,
      method: 'GET',
      bodyKind: 'none',
      bodyValue: 'inactive content',
      grace_ms: 0,
    });
    expect(output.body).toEqual({ kind: 'none' });
    expect(output.pacing_ms).toBe(0);
    expect(output.grace_ms).toBe(0);
  });

  it.each([
    'ftp://127.0.0.1/items',
    'http://user:password@127.0.0.1/items',
    'http://127.0.0.1/items#fragment',
    'http://127.0.0.1/items#',
    '/relative',
  ])('rejects unsupported URL %s', (url) => {
    expect(
      v.safeParse(requestEditorSchema, {
        ...input,
        url,
      }).success,
    ).toBe(false);
  });

  it.each([
    { bodyValue: '{' },
    { method: 'GET' },
    { vus: '257' },
    { vus: '1.5' },
    { duration_ms: '0' },
    { timeout_ms: '3600001' },
    { timeout_ms: '-1' },
    { pacing_ms: '-1' },
    { grace_ms: '4294967296' },
    { max_request_bytes: '' },
  ])('rejects invalid workload or body settings %j', (change) => {
    expect(
      v.safeParse(requestEditorSchema, {
        ...input,
        ...change,
      }).success,
    ).toBe(false);
  });

  it.each(['Host', 'Content-Length', 'TRANSFER-ENCODING', 'Invalid Header'])(
    'rejects managed or malformed header %s',
    (name) => {
      expect(
        v.safeParse(requestEditorSchema, {
          ...input,
          headers: [
            {
              name,
              value: 'value',
            },
          ],
        }).success,
      ).toBe(false);
    },
  );

  it('rejects header line injection and attributes JSON failures to the body field', () => {
    expect(
      v.safeParse(requestEditorSchema, {
        ...input,
        headers: [
          {
            name: 'X-Test',
            value: 'value\r\nInjected: true',
          },
        ],
      }).success,
    ).toBe(false);

    const invalid = v.safeParse(requestEditorSchema, {
      ...input,
      bodyValue: '{',
    });
    expect(invalid.success).toBe(false);
    expect(invalid.issues?.[0]?.path?.[0]?.key).toBe('bodyValue');
  });
});

describe('ramping workload editor', () => {
  it('converts zero starting VUs and ordered ramp/hold/zero stages without inactive constant settings', () => {
    const output = v.parse(requestEditorSchema, {
      ...input,
      workloadKind: 'ramping',
      vus: '',
      duration_ms: '',
      start_vus: '0',
      stages: [
        {
          duration_ms: '1000',
          target_vus: '10',
        },
        {
          duration_ms: '2000',
          target_vus: '10',
        },
        {
          duration_ms: '1000',
          target_vus: '0',
        },
      ],
    });
    expect(output.workload).toEqual({
      kind: 'ramping',
      start_vus: 0,
      stages: [
        {
          duration_ms: 1000,
          target_vus: 10,
        },
        {
          duration_ms: 2000,
          target_vus: 10,
        },
        {
          duration_ms: 1000,
          target_vus: 0,
        },
      ],
    });
    expect(output).not.toHaveProperty('vus');
    expect(output).not.toHaveProperty('duration_ms');
  });

  it.each([
    { stages: [] },
    {
      stages: [
        {
          duration_ms: 0,
          target_vus: 1,
        },
      ],
    },
    {
      stages: [
        {
          duration_ms: 1000,
          target_vus: -1,
        },
      ],
    },
    {
      stages: [
        {
          duration_ms: 1000,
          target_vus: 1.5,
        },
      ],
    },
    {
      stages: [
        {
          duration_ms: 1000,
          target_vus: 257,
        },
      ],
    },
    { start_vus: 257 },
    { start_vus: -1 },
    {
      stages: [
        {
          duration_ms: 4_294_967_295,
          target_vus: 0,
        },
        {
          duration_ms: 1,
          target_vus: 0,
        },
      ],
    },
  ])('rejects invalid ramp settings %j', (change) => {
    expect(
      v.safeParse(requestEditorSchema, {
        ...input,
        workloadKind: 'ramping',
        stages: [
          {
            duration_ms: '1000',
            target_vus: '0',
          },
        ],
        ...change,
      }).success,
    ).toBe(false);
  });

  it('ignores unfinished stage edits when switching back to a constant workload', () => {
    const output = v.parse(requestEditorSchema, {
      ...input,
      start_vus: '',
      stages: [
        {
          duration_ms: '',
          target_vus: '-5',
        },
      ],
    });
    expect(output.workload).toEqual({
      kind: 'constant',
      vus: 2,
      duration_ms: 1000,
    });
  });
});
