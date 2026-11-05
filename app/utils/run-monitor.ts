import type { ComputedRef, ShallowRef } from 'vue';
import type { RunConfig, RunConfirmation, RunSnapshot, RunUpdate } from '../types/contracts';
import * as v from 'valibot';

type ChartPoint = {
  seconds: number;
  requests: number | null;
  latency: number | null;
  failures: number | null;
  vus: number | null;
  desiredVus: number | null;
};

type RunTransport = {
  prepare: (config: RunConfig) => Promise<RunConfirmation>;
  start: (config: RunConfig, interval: number) => Promise<RunSnapshot>;
  stop: (runId: string) => Promise<RunSnapshot>;
  current: () => Promise<RunSnapshot | null>;
  listen: (receive: (update: RunUpdate) => void) => Promise<() => void>;
};

type RunMonitor = {
  snapshot: ShallowRef<RunSnapshot | null>;
  points: ShallowRef<ChartPoint[]>;
  busy: ShallowRef<boolean>;
  error: ShallowRef<string | null>;
  connected: ShallowRef<boolean>;
  gaps: ShallowRef<boolean>;
  active: ComputedRef<boolean>;
  prepare: (config: RunConfig) => Promise<RunConfirmation | null>;
  start: (config: RunConfig, interval: number) => Promise<void>;
  stop: () => Promise<void>;
  recover: () => Promise<void>;
  connect: () => Promise<void>;
  dispose: () => void;
};

const nativeErrorSchema = v.object({
  code: v.string(),
  message: v.string(),
});
const terminalStates = new Set(['completed', 'cancelled', 'failed']);
const pointLimit = 600;
const stateOrder: Record<RunSnapshot['state'], number> = {
  starting: 0,
  running: 1,
  stopping: 2,
  completed: 3,
  cancelled: 3,
  failed: 3,
};

/**
 * Owns the current desktop run, bounded chart data, and recoverable subscriptions.
 * @param transport - Typed native commands and aggregate event subscription.
 * @returns Reactive state and actions. Failures populate error without exposing transport details. Call dispose when the application unmounts.
 * @example
 * ```typescript
 * const monitor = createRunMonitor(transport)
 * await monitor.connect()
 * await monitor.recover()
 * monitor.dispose()
 * ```
 */
export function createRunMonitor(transport: RunTransport): RunMonitor {
  const snapshot = shallowRef<RunSnapshot | null>(null);
  const points = shallowRef<ChartPoint[]>([]);
  const busy = shallowRef<boolean>(false);
  const error = shallowRef<string | null>(null);
  const connected = shallowRef<boolean>(false);
  const gaps = shallowRef<boolean>(false);
  const active = computed<boolean>(
    () => snapshot.value !== null && !terminalStates.has(snapshot.value.state),
  );

  let sequence = 0n;
  let disposed = false;
  let unlisten: (() => void) | null = null;
  let connecting: Promise<void> | null = null;
  let recovering: Promise<void> | null = null;

  function report(failure: v.InferOutput<typeof nativeErrorSchema> | null): void {
    error.value = failure
      ? `${failure.code}: ${failure.message}`
      : 'IPC_UNAVAILABLE: Open Velo in the desktop application and try again.';
  }

  function accept(next: RunSnapshot): boolean {
    if (disposed) {
      return false;
    }

    const previous = snapshot.value;
    if (previous && BigInt(next.run_id) < BigInt(previous.run_id)) {
      return false;
    }

    if (previous?.run_id === next.run_id) {
      if (
        BigInt(next.actual_us) < BigInt(previous.actual_us) ||
        stateOrder[next.state] < stateOrder[previous.state]
      ) {
        return false;
      }
    } else {
      points.value = [];
      sequence = 0n;
      gaps.value = false;
    }

    snapshot.value = next;
    return true;
  }

  function receive(update: RunUpdate): void {
    const nextSequence = BigInt(update.sequence);
    if (snapshot.value?.run_id === update.snapshot.run_id && nextSequence <= sequence) {
      return;
    }

    if (
      disposed ||
      (snapshot.value && BigInt(update.snapshot.run_id) < BigInt(snapshot.value.run_id))
    ) {
      return;
    }

    accept(update.snapshot);

    const seconds = safeNumber(update.snapshot.actual_us);
    const interval = safeNumber(update.interval_us);
    if (seconds === null || interval === null) {
      gaps.value = true;
      sequence = nextSequence;
      return;
    }

    if (interval === 0) {
      sequence = nextSequence;
      return;
    }

    const nextPoints = points.value.slice();
    const previousPoint = nextPoints.at(-1);
    if (nextSequence !== sequence + 1n) {
      gaps.value = true;
      nextPoints.push({
        seconds: previousPoint ? (previousPoint.seconds + seconds / 1e6) / 2 : 0,
        requests: null,
        latency: null,
        failures: null,
        vus: null,
        desiredVus: null,
      });
    }

    const requests = safeNumber(update.interval_requests);
    const failures = safeNumber(update.interval_failures);
    nextPoints.push({
      seconds: seconds / 1e6,
      requests: requests === null ? null : requests / (interval / 1e6),
      latency:
        update.response_latency.p95_us === null ? null : update.response_latency.p95_us / 1000,
      failures:
        requests === null || failures === null || requests === 0
          ? null
          : (failures / requests) * 100,
      vus: update.snapshot.active_vus,
      desiredVus: update.snapshot.desired_vus,
    });
    points.value = nextPoints.slice(-pointLimit);
    sequence = nextSequence;
  }

  async function connect(): Promise<void> {
    if (disposed || unlisten) {
      return;
    }

    if (connecting) {
      return connecting;
    }

    connecting = (async () => {
      try {
        const cleanup = await transport.listen(receive);
        if (disposed) {
          cleanup();
        } else {
          unlisten = cleanup;
          connected.value = true;
        }
      } catch {
        connected.value = false;
      }
    })();
    await connecting;
    connecting = null;
  }

  async function recover(): Promise<void> {
    if (disposed) {
      return;
    }

    if (recovering) {
      return recovering;
    }

    recovering = (async () => {
      try {
        const current = await transport.current();
        if (current) {
          accept(current);
        }
        if (error.value?.startsWith('RUN_RECOVERY_FAILED:')) {
          error.value = null;
        }
      } catch {
        if (!disposed) {
          error.value = 'RUN_RECOVERY_FAILED: Native state is unavailable. Retrying automatically.';
        }
      }
    })();
    await recovering;
    recovering = null;
  }

  async function prepare(config: RunConfig): Promise<RunConfirmation | null> {
    if (busy.value || active.value || disposed) {
      return null;
    }

    busy.value = true;
    error.value = null;
    try {
      return await transport.prepare(config);
    } catch (error_) {
      const parsed = v.safeParse(nativeErrorSchema, error_);
      report(parsed.success ? parsed.output : null);
      return null;
    } finally {
      busy.value = false;
    }
  }

  async function start(config: RunConfig, interval: number): Promise<void> {
    if (busy.value || active.value || disposed) {
      return;
    }

    busy.value = true;
    error.value = null;
    try {
      await connect();
      if (!connected.value) {
        error.value = 'RUN_SUBSCRIPTION_FAILED: Live monitoring could not connect. Try again.';
        return;
      }

      accept(await transport.start(config, interval));
    } catch (error_) {
      const parsed = v.safeParse(nativeErrorSchema, error_);
      report(parsed.success ? parsed.output : null);
      await recover();
    } finally {
      busy.value = false;
    }
  }

  async function stop(): Promise<void> {
    const current = snapshot.value;
    if (!current || !active.value || busy.value || disposed) {
      return;
    }

    busy.value = true;
    error.value = null;
    try {
      accept(await transport.stop(current.run_id));
    } catch (error_) {
      const parsed = v.safeParse(nativeErrorSchema, error_);
      report(parsed.success ? parsed.output : null);
    } finally {
      busy.value = false;
    }
  }

  function dispose(): void {
    disposed = true;
    unlisten?.();
    unlisten = null;
    connected.value = false;
  }

  return {
    snapshot,
    points,
    busy,
    error,
    connected,
    gaps,
    active,
    prepare,
    start,
    stop,
    recover,
    connect,
    dispose,
  };
}

function safeNumber(decimal: string): number | null {
  const value = BigInt(decimal);
  return value <= BigInt(Number.MAX_SAFE_INTEGER) ? Number(value) : null;
}
