import type {
  LatencySnapshot,
  RunConfig,
  RunSnapshot,
  RunUpdate,
} from '../../../app/types/contracts';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { computed, shallowRef } from 'vue';
import { createRunMonitor } from '../../../app/utils/run-monitor';

beforeEach(() => {
  vi.stubGlobal('shallowRef', shallowRef);
  vi.stubGlobal('computed', computed);
});

afterEach(() => vi.unstubAllGlobals());

const population: LatencySnapshot = {
  significant_digits: 3,
  lowest_us: 1,
  highest_us: 3_600_000_000,
  samples: '1',
  out_of_range: '0',
  mean_us: 1000,
  min_us: 1000,
  max_us: 1000,
  median_us: 1000,
  p90_us: 1000,
  p95_us: 1000,
  p99_us: 1000,
};

function snapshot(runId = '1', seconds = 1): RunSnapshot {
  return {
    dataset: null,
    thresholds: [],
    run_id: runId,
    state: 'running',
    confirmation: {
      origin: 'http://127.0.0.1:8080',
      vus: 1,
      duration_ms: 60_000,
      workload: {
        kind: 'constant',
        vus: 1,
        duration_ms: 60_000,
      },
      budget: {
        max_vus: 1000,
        max_request_bytes: 16_777_216,
      },
      dataset: null,
      warnings: [],
    },
    scheduling_us: String(seconds * 1e6),
    drain_us: '0',
    actual_us: String(seconds * 1e6),
    desired_vus: 1,
    retiring_vus: 0,
    active_vus: 1,
    max_vus: 1,
    failure_code: null,
    arrival: {
      offered: '0',
      capacity_drops: '0',
      dataset_drops: '0',
      missed_deadlines: '0',
      max_lateness_us: '0',
    },
    metrics: {
      counters: {
        started: '9007199254740993',
        responses: '10',
        successes: '9',
        status_failures: '1',
        transport_failures: '0',
        timeouts: '0',
        cancelled: '0',
        in_flight: '0',
        response_body_bytes: '100',
        completed_iterations: '10',
        failed_iterations: '1',
      },
      response_latency: population,
      transport_latency: population,
      timeout_latency: population,
      http_failure_rate: 0.1,
      request_rate: 10,
      iteration_rate: 10,
      success_rate: 9,
      aggregation_wait_us: '0',
      aggregation_interference: false,
      counter_overflow: false,
      max_in_flight: '1',
    },
  };
}

function harness() {
  let receive: ((update: RunUpdate) => void) | null = null;
  let cleaned = 0;
  let current = snapshot();
  const monitor = createRunMonitor({
    prepare: () => Promise.resolve(current.confirmation),
    start: () => Promise.resolve(current),
    stop: () =>
      Promise.resolve({
        ...current,
        state: 'stopping',
      }),
    current: () => Promise.resolve(current),
    listen: (callback) => {
      receive = callback;
      return Promise.resolve(() => {
        cleaned += 1;
      });
    },
  });

  function emit(sequence: number, runId = '1'): void {
    receive?.({
      sequence: String(sequence),
      snapshot: snapshot(runId, sequence),
      interval_us: '1000000',
      interval_requests: '10',
      interval_failures: '1',
      response_latency: population,
    });
  }

  return {
    monitor,
    emit,
    cleaned: () => cleaned,
    setCurrent: (value: RunSnapshot) => {
      current = value;
    },
  };
}

describe('run monitor', () => {
  it('subscribes before starting, prevents overlapping actions, and preserves stop state', async () => {
    const events: string[] = [];
    const accepted = Promise.withResolvers<RunSnapshot>();
    const config: RunConfig = {
      url: 'http://127.0.0.1:8080/',
      method: 'GET',
      auth: { kind: 'none' },
      protocol: 'auto',
      max_redirects: 0,
      headers: [],
      query: [],
      thresholds: [],
      body: { kind: 'none' },
      workload: {
        kind: 'constant',
        vus: 1,
        duration_ms: 1000,
      },
      timeout_ms: 30000,
      pacing_ms: 0,
      grace_ms: 5000,
      budget: snapshot().confirmation.budget,
    };
    const monitor = createRunMonitor({
      prepare: () => Promise.resolve(snapshot().confirmation),
      start: (_config, interval) => {
        expect(interval).toBe(1000);
        events.push('start');
        return accepted.promise;
      },
      stop: (runId) => {
        expect(runId).toBe('1');
        return Promise.resolve({
          ...snapshot(),
          state: 'stopping',
        });
      },
      current: () => Promise.resolve(null),
      listen: () => {
        events.push('subscribe');
        return Promise.resolve(vi.fn<() => void>());
      },
    });
    expect(await monitor.prepare(config)).toEqual(snapshot().confirmation);
    const starting = monitor.start(config, 1000);
    await monitor.start(config, 1000);
    expect(monitor.busy.value).toBe(true);
    accepted.resolve(snapshot());
    await starting;
    expect(events).toEqual(['subscribe', 'start']);
    await monitor.stop();
    expect(monitor.snapshot.value?.state).toBe('stopping');
    expect(monitor.active.value).toBe(true);
    monitor.dispose();
  });

  it('retries subscription and state recovery without exposing infrastructure errors', async () => {
    let available = false;
    const monitor = createRunMonitor({
      prepare: () => Promise.reject(new Error('private target')),
      start: () => Promise.reject(new Error('private target')),
      stop: () => Promise.reject(new Error('private target')),
      current: () =>
        available ? Promise.resolve(snapshot()) : Promise.reject(new Error('private target')),
      listen: () =>
        available
          ? Promise.resolve(vi.fn<() => void>())
          : Promise.reject(new Error('private target')),
    });
    await monitor.connect();
    await monitor.recover();
    expect(monitor.connected.value).toBe(false);
    expect(monitor.error.value).toContain('RUN_RECOVERY_FAILED:');
    expect(monitor.error.value).not.toContain('private');
    available = true;
    await monitor.connect();
    await monitor.recover();
    expect(monitor.connected.value).toBe(true);
    expect(monitor.error.value).toBeNull();
    expect(monitor.snapshot.value?.state).toBe('running');
    monitor.dispose();
  });

  it('uses exact cumulative strings and interval rates; ignores duplicate and stale events', async () => {
    const { monitor, emit } = harness();
    await monitor.connect();
    emit(1);
    emit(1);
    expect(monitor.points.value).toHaveLength(1);
    expect(monitor.points.value[0]).toMatchObject({
      requests: 10,
      failures: 10,
      latency: 1,
    });
    expect(monitor.snapshot.value?.metrics.counters.started).toBe('9007199254740993');
    emit(1, '2');
    emit(99, '1');
    expect(monitor.snapshot.value?.run_id).toBe('2');
    expect(monitor.points.value).toHaveLength(1);
    monitor.dispose();
  });

  it('marks missed intervals and retains at most 600 points over extended runs', async () => {
    const { monitor, emit } = harness();
    await monitor.connect();
    emit(1);
    emit(3);
    expect(monitor.gaps.value).toBe(true);
    expect(monitor.points.value[1]?.requests).toBeNull();
    for (let sequence = 4; sequence < 100_000; sequence += 1) {
      emit(sequence);
    }
    expect(monitor.points.value).toHaveLength(600);
    expect(monitor.points.value.at(-1)?.seconds).toBe(99_999);
    monitor.dispose();
  });

  it('recovers missed terminal state without fabricating chart intervals or regressing state', async () => {
    const { monitor, emit, setCurrent } = harness();
    await monitor.connect();
    emit(1);
    setCurrent({
      ...snapshot(),
      state: 'starting',
    });
    await monitor.recover();
    expect(monitor.snapshot.value?.state).toBe('running');
    setCurrent({
      ...snapshot('1', 5),
      state: 'completed',
      active_vus: 0,
    });
    await monitor.recover();
    expect(monitor.active.value).toBe(false);
    expect(monitor.snapshot.value?.state).toBe('completed');
    expect(monitor.points.value).toHaveLength(1);
    emit(4);
    expect(monitor.snapshot.value?.state).toBe('completed');
    expect(monitor.points.value.at(-1)?.seconds).toBe(4);
    monitor.dispose();
  });

  it('cleans subscriptions once and ignores deliveries after disposal', async () => {
    const { monitor, emit, cleaned } = harness();
    await Promise.all([monitor.connect(), monitor.connect()]);
    monitor.dispose();
    monitor.dispose();
    emit(1);
    expect(cleaned()).toBe(1);
    expect(monitor.snapshot.value).toBeNull();
    expect(monitor.connected.value).toBe(false);
  });

  it('cleans a listener that resolves after the application unmounts', async () => {
    const subscription = Promise.withResolvers<() => void>();
    let cleaned = false;
    const monitor = createRunMonitor({
      prepare: () => Promise.reject(new Error('private target')),
      start: () => Promise.reject(new Error('private target')),
      stop: () => Promise.reject(new Error('private target')),
      current: () => Promise.resolve(null),
      listen: () => subscription.promise,
    });
    const connection = monitor.connect();
    monitor.dispose();
    subscription.resolve(() => {
      cleaned = true;
    });
    await connection;
    expect(cleaned).toBe(true);
    expect(monitor.connected.value).toBe(false);
  });
});

describe('ramping monitor', () => {
  it('keeps desired and actual workers distinct through retirement and missing intervals', async () => {
    let receive: ((update: RunUpdate) => void) | null = null;
    const monitor = createRunMonitor({
      prepare: () => Promise.resolve(snapshot().confirmation),
      start: () => Promise.resolve(snapshot()),
      stop: () => Promise.resolve(snapshot()),
      current: () => Promise.resolve(null),
      listen: (callback) => {
        receive = callback;
        return Promise.resolve(vi.fn<() => void>());
      },
    });
    await monitor.connect();
    function emit(sequence: string, desired: number, actual: number, retiring: number): void {
      receive?.({
        sequence,
        snapshot: {
          ...snapshot('1', Number(sequence)),
          desired_vus: desired,
          active_vus: actual,
          retiring_vus: retiring,
        },
        interval_us: '1000000',
        interval_requests: '10',
        interval_failures: '0',
        response_latency: population,
      });
    }
    emit('1', 4, 4, 0);
    emit('3', 0, 4, 4);
    expect(monitor.snapshot.value?.retiring_vus).toBe(4);
    expect(monitor.points.value.map(({ vus, desiredVus }) => [vus, desiredVus])).toEqual([
      [4, 4],
      [null, null],
      [4, 0],
    ]);
    monitor.dispose();
  });
});
