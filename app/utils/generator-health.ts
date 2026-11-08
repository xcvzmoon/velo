import type { HostTelemetry, RunSnapshot, RunUpdate } from '../types/contracts';

export type GeneratorArrivalRates = {
  offered: number | null;
  started: number | null;
};

export type GeneratorRequestRates = {
  started: number | null;
  completed: number | null;
};

export type GeneratorResourcePoint = {
  seconds: number;
  telemetry: HostTelemetry | null;
};

export type ResourcePressure = {
  evidence: string[];
  pressure: number | null;
};

export type GeneratorObservation = ResourcePressure & { seconds: number };

export type GeneratorPoint = {
  seconds: number;
  telemetry: HostTelemetry | null;
  capacityDrops: string | null;
  missedDeadlines: string | null;
  vus: number | null;
  desiredVus: number | null;
  retiringVus: number | null;
};

/**
 * Maps retained aggregate samples to generator evidence on the workload timeline.
 * @param updates - Ordered retained updates, including any history compaction.
 * @returns Bounded points with null evidence at sequence gaps; no missing intervals are interpolated.
 * @example
 * ```typescript
 * const points = storedGeneratorPoints(entry.points)
 * ```
 */
export function storedGeneratorPoints(updates: RunUpdate[]): GeneratorPoint[] {
  const points: GeneratorPoint[] = [];
  let previous: RunUpdate | null = null;
  for (const update of updates) {
    const seconds = Number(update.snapshot.actual_us) / 1e6;
    if (previous && BigInt(update.sequence) !== BigInt(previous.sequence) + 1n) {
      points.push({
        seconds: (Number(previous.snapshot.actual_us) / 1e6 + seconds) / 2,
        telemetry: null,
        capacityDrops: null,
        missedDeadlines: null,
        vus: null,
        desiredVus: null,
        retiringVus: null,
      });
    }
    points.push({
      seconds,
      telemetry: update.snapshot.telemetry,
      capacityDrops: update.snapshot.arrival.capacity_drops,
      missedDeadlines: update.snapshot.arrival.missed_deadlines,
      vus: update.snapshot.active_vus,
      desiredVus: update.snapshot.desired_vus,
      retiringVus: update.snapshot.retiring_vus,
    });
    previous = update;
  }
  return points;
}

/**
 * Lists observed resource pressure without assigning a cause to target failures.
 * @param telemetry - Latest native sample, or null for missing data.
 * @param seconds - Workload-relative observation time in seconds.
 * @returns Evidence with explicit thresholds and a marker: one for pressure, zero for no listed evidence, or null for unavailable data. Zero does not prove spare capacity.
 * @example
 * ```typescript
 * const evidence = resourcePressure(snapshot.telemetry, Number(snapshot.actual_us) / 1e6)
 * ```
 */
export function resourcePressure(
  telemetry: HostTelemetry | null,
  seconds: number,
): ResourcePressure {
  if (!isTelemetryAvailable(telemetry, seconds)) {
    return {
      evidence: ['Resource measurements unavailable'],
      pressure: null,
    };
  }
  const evidence: string[] = [];
  if (telemetry.process_cpu_percent !== null && telemetry.process_cpu_percent >= 90) {
    evidence.push(`Native process CPU ${telemetry.process_cpu_percent.toFixed(1)}% (≥90%)`);
  }
  if (telemetry.system_cpu_percent !== null && telemetry.system_cpu_percent >= 90) {
    evidence.push(`Host CPU ${telemetry.system_cpu_percent.toFixed(1)}% (≥90%)`);
  }
  if (
    telemetry.system_available_memory_bytes !== null &&
    telemetry.system_total_memory_bytes !== null
  ) {
    const fraction =
      Number(telemetry.system_available_memory_bytes) / Number(telemetry.system_total_memory_bytes);
    if (fraction <= 0.1) {
      evidence.push(`Host available RAM ${(fraction * 100).toFixed(1)}% (≤10%)`);
    }
  }
  const complete =
    telemetry.process_cpu_percent !== null &&
    telemetry.process_memory_bytes !== null &&
    telemetry.system_cpu_percent !== null &&
    telemetry.system_available_memory_bytes !== null &&
    telemetry.system_total_memory_bytes !== null;
  const pressure = evidence.length > 0 ? 1 : complete ? 0 : null;
  if (!complete) {
    evidence.push('Some resource measurements unavailable');
  }
  return { evidence, pressure };
}

/**
 * Rejects missing, future, and stale native samples without inventing zero usage.
 * @param telemetry - Native sample to inspect.
 * @param seconds - Workload-relative observation time in seconds.
 * @returns True for an available sample no more than 2.5 seconds old.
 * @example
 * ```typescript
 * const available = isTelemetryAvailable(snapshot.telemetry, Number(snapshot.actual_us) / 1e6)
 * ```
 */
export function isTelemetryAvailable(
  telemetry: HostTelemetry | null,
  seconds: number,
): telemetry is HostTelemetry {
  if (telemetry?.status !== 'available' || telemetry.sampled_at_us === null) {
    return false;
  }
  const age = seconds - Number(telemetry.sampled_at_us) / 1e6;
  return age >= 0 && age <= 2.5;
}

/**
 * Calculates rates that share the full scheduling-plus-drain observation window.
 * @param snapshot - Authoritative cumulative run accounting.
 * @returns Started and completed HTTP request rates, or null for an empty or overflowed window.
 * @example
 * ```typescript
 * const rates = generatorRequestRates(snapshot)
 * ```
 */
export function generatorRequestRates(snapshot: RunSnapshot): GeneratorRequestRates {
  const seconds = Number(snapshot.actual_us) / 1e6;
  if (seconds <= 0 || snapshot.metrics.counter_overflow) {
    return {
      started: null,
      completed: null,
    };
  }
  return {
    started: Number(snapshot.metrics.counters.started) / seconds,
    completed: snapshot.metrics.request_rate,
  };
}

/**
 * Deduplicates native samples and leaves gaps where the sampler has no recent observation.
 * @param points - Ordered aggregate observations, with explicit null points for delivery gaps.
 * @returns Native timestamped samples and gaps; each native sample is plotted once.
 * @example
 * ```typescript
 * const samples = generatorResourcePoints(points)
 * ```
 */
export function generatorResourcePoints(points: GeneratorPoint[]): GeneratorResourcePoint[] {
  const result: GeneratorResourcePoint[] = [];
  let previous = -1;
  for (const point of points) {
    if (!isTelemetryAvailable(point.telemetry, point.seconds)) {
      if (point.seconds > previous) {
        result.push({
          seconds: point.seconds,
          telemetry: null,
        });
        previous = point.seconds;
      }
      continue;
    }
    const seconds = Number(point.telemetry.sampled_at_us) / 1e6;
    if (seconds <= previous) {
      continue;
    }
    if (previous >= 0 && seconds - previous > 2.5) {
      result.push({
        seconds: (previous + seconds) / 2,
        telemetry: null,
      });
    }
    result.push({
      seconds,
      telemetry: point.telemetry,
    });
    previous = seconds;
  }
  return result;
}

/**
 * Combines native resource evidence with separate arrival-admission evidence.
 * @param points - Ordered workload observations; null points prevent deltas across missing intervals.
 * @returns A pressure timeline with unknown intervals kept null and evidence tied to each observation.
 * @example
 * ```typescript
 * const observations = generatorObservations(points)
 * ```
 */
export function generatorObservations(points: GeneratorPoint[]): GeneratorObservation[] {
  const result: GeneratorObservation[] = [];
  let previous: GeneratorPoint | null = null;
  for (const point of points) {
    const resource = resourcePressure(point.telemetry, point.seconds);
    const evidence = resource.evidence;
    let schedulingPressure = false;
    for (const [field, label] of [
      ['capacityDrops', 'capacity drops'],
      ['missedDeadlines', 'missed arrival slots'],
    ] as const) {
      const before = previous?.[field];
      const after = point[field];
      if (before !== null && before !== undefined && after !== null) {
        const delta = BigInt(after) - BigInt(before);
        if (delta > 0n) {
          evidence.push(`${delta} ${label} since preceding retained sample`);
          schedulingPressure = true;
        }
      }
    }
    result.push({
      seconds: point.seconds,
      evidence,
      pressure: schedulingPressure ? 1 : resource.pressure,
    });
    previous = point;
  }
  return result;
}

/**
 * Calculates offered and admitted iterations over the same scheduling-only window.
 * @param snapshot - Native arrival counters, independent of HTTP outcomes and scenario step counts.
 * @returns Iteration rates, or null when the window is empty or accounting is unavailable.
 * @example
 * ```typescript
 * const rates = generatorArrivalRates(snapshot)
 * ```
 */
export function generatorArrivalRates(snapshot: RunSnapshot): GeneratorArrivalRates {
  const seconds = Number(snapshot.scheduling_us) / 1e6;
  if (
    seconds <= 0 ||
    snapshot.metrics.counter_overflow ||
    snapshot.confirmation.workload.kind !== 'arrival'
  ) {
    return {
      offered: null,
      started: null,
    };
  }
  const arrival = snapshot.arrival;
  const started =
    BigInt(arrival.offered) -
    BigInt(arrival.capacity_drops) -
    BigInt(arrival.missed_deadlines) -
    BigInt(arrival.dataset_drops);
  return {
    offered: Number(arrival.offered) / seconds,
    started: started >= 0n ? Number(started) / seconds : null,
  };
}
