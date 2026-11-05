import type { RunConfirmation, RunSnapshot, RunUpdate } from '../types/contracts';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export default defineNuxtPlugin((nuxtApp) => {
  const runs = createRunMonitor({
    prepare: (config) => invoke<RunConfirmation>('prepare_run', { config }),
    start: (config, interval) =>
      invoke<RunSnapshot>('start_run', {
        config,
        authorized: true,
        updateIntervalMs: interval,
      }),
    stop: (runId) => invoke<RunSnapshot>('stop_run', { runId }),
    current: () => invoke<RunSnapshot | null>('current_run'),
    listen: (receive) =>
      listen<RunUpdate>('run-update', ({ payload }) => {
        receive(payload);
      }),
  });
  let timer: ReturnType<typeof setTimeout> | null = null;
  let disposed = false;

  async function recover(): Promise<void> {
    await runs.connect();
    await runs.recover();
    if (!disposed) {
      timer = setTimeout(() => {
        void recover();
      }, 2000);
    }
  }

  nuxtApp.vueApp.onUnmount(() => {
    disposed = true;
    if (timer !== null) {
      clearTimeout(timer);
    }
    runs.dispose();
  });
  void recover();

  return { provide: { runs } };
});
