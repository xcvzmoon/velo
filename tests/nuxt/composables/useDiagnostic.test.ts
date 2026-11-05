import * as v from 'valibot';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { computed, shallowRef } from 'vue';
import { useDiagnostic } from '../../../app/composables/useDiagnostic';
import { diagnosticInputSchema } from '../../../app/schemas/diagnostic';

beforeEach(() => {
  vi.stubGlobal('shallowRef', shallowRef);
  vi.stubGlobal('computed', computed);
});
afterEach(() => {
  vi.unstubAllGlobals();
});

const report = {
  engine: 'velo-engine',
  version: '0.0.0',
};

describe('useDiagnostic', () => {
  it('validates labels with the schema used by Formisch', () => {
    expect(v.safeParse(diagnosticInputSchema, { label: ' ' }).success).toBe(false);
    expect(v.parse(diagnosticInputSchema, { label: ' Desktop ' })).toEqual({ label: 'Desktop' });
    expect(v.safeParse(diagnosticInputSchema, { label: '🚀'.repeat(64) }).success).toBe(true);
    expect(v.safeParse(diagnosticInputSchema, { label: '🚀'.repeat(65) }).success).toBe(false);
  });

  it('exposes the returned engine identity', async () => {
    const diagnostic = useDiagnostic((input) => {
      expect(input.label).toBe('Desktop');
      return Promise.resolve(report);
    });
    await diagnostic.checkConnection({ label: 'Desktop' });
    expect(diagnostic.result.value).toEqual(report);
    expect(diagnostic.error.value).toBeNull();
    expect(diagnostic.message.value).toBe('Connected to velo-engine 0.0.0.');
  });

  it('shows structured errors and hides infrastructure details', async () => {
    const diagnostic = useDiagnostic(() =>
      Promise.reject({
        code: 'INVALID_DIAGNOSTIC_LABEL',
        message: 'Invalid label',
        field: 'label',
      }),
    );
    await diagnostic.checkConnection({ label: 'Desktop' });
    expect(diagnostic.error.value).toBe('INVALID_DIAGNOSTIC_LABEL: Invalid label');
    const unavailable = useDiagnostic(() => Promise.reject(new Error('sensitive details')));
    await unavailable.checkConnection({ label: 'Desktop' });
    expect(unavailable.error.value).toBe(
      'IPC_UNAVAILABLE: Open Velo in the desktop application and try again.',
    );
    expect(unavailable.pending.value).toBe(false);
  });

  it('prevents overlapping checks, resets pending, and allows retry', async () => {
    const { promise, resolve } = Promise.withResolvers<typeof report>();
    let calls = 0;
    const diagnostic = useDiagnostic(() => {
      calls += 1;
      return promise;
    });
    const first = diagnostic.checkConnection({ label: 'Desktop' });
    expect(diagnostic.pending.value).toBe(true);
    await diagnostic.checkConnection({ label: 'Desktop' });
    expect(calls).toBe(1);
    resolve(report);
    await first;
    expect(diagnostic.pending.value).toBe(false);
    const second = diagnostic.checkConnection({ label: 'Desktop' });
    expect(calls).toBe(2);
    resolve(report);
    await second;
  });
});
