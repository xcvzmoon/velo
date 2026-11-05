import type { ComputedRef, ShallowRef } from 'vue';
import type { DiagnosticInput, DiagnosticReport } from '../types/contracts';
import { invoke } from '@tauri-apps/api/core';
import * as v from 'valibot';

type DiagnosticTransport = (input: DiagnosticInput) => Promise<DiagnosticReport>;

type DiagnosticState = {
  pending: ShallowRef<boolean>;
  result: ShallowRef<DiagnosticReport | null>;
  error: ShallowRef<string | null>;
  message: ComputedRef<string>;
  checkConnection: (input: DiagnosticInput) => Promise<void>;
};

const commandErrorSchema = v.object({
  code: v.string(),
  message: v.string(),
  field: v.nullable(v.string()),
});

/**
 * Tracks the result of diagnostic submissions validated by the Formisch form.
 * @param transport - Native invocation, replaceable in tests without module mocking.
 * @returns Independent request state and a check action. Failures populate error; the action does not throw transport errors. Concurrent checks are ignored. No HTTP traffic is sent.
 * @example
 * ```typescript
 * const { pending, message, checkConnection } = useDiagnostic()
 * await checkConnection({ label: 'Desktop' })
 * ```
 */
export function useDiagnostic(transport: DiagnosticTransport = invokeDiagnostic): DiagnosticState {
  const pending = shallowRef<boolean>(false);
  const result = shallowRef<DiagnosticReport | null>(null);
  const error = shallowRef<string | null>(null);

  const message = computed<string>(() => {
    if (pending.value) {
      return 'Checking the native connection…';
    }

    if (error.value) {
      return error.value;
    }

    if (result.value) {
      return `Connected to ${result.value.engine} ${result.value.version}.`;
    }

    return 'The native connection has not been checked.';
  });

  async function checkConnection(input: DiagnosticInput): Promise<void> {
    if (pending.value) {
      return;
    }

    error.value = null;
    result.value = null;
    pending.value = true;

    try {
      result.value = await transport(input);
    } catch (error_) {
      const parsedError = v.safeParse(commandErrorSchema, error_);
      error.value = parsedError.success
        ? `${parsedError.output.code}: ${parsedError.output.message}`
        : 'IPC_UNAVAILABLE: Open Velo in the desktop application and try again.';
    } finally {
      pending.value = false;
    }
  }

  return {
    pending,
    result,
    error,
    message,
    checkConnection,
  };
}

function invokeDiagnostic(input: DiagnosticInput): Promise<DiagnosticReport> {
  return invoke<DiagnosticReport>('diagnose', { input });
}
