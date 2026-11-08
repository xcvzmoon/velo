<script setup lang="ts">
  import type { RunSnapshot } from '~/types/contracts';

  const props = defineProps<{ storedSnapshot?: RunSnapshot }>();

  const { $runs } = useNuxtApp();
  const headingId = useId();
  const snapshot = computed<RunSnapshot | null>(() => props.storedSnapshot ?? $runs.snapshot.value);
  const active = computed<boolean>(() => (props.storedSnapshot ? false : $runs.active.value));
</script>

<template>
  <section
    v-if="snapshot && !active"
    class="mt-8 space-y-6 rounded-lg border p-6"
    :aria-labelledby="headingId"
  >
    <div>
      <h2
        :id="headingId"
        class="text-lg font-medium"
      >
        Final summary
      </h2>

      <p class="text-muted-foreground mt-2 text-sm">
        {{
          snapshot.measurement.complete
            ? 'Scheduling completed.'
            : 'Partial run. Measurements cover observed work only.'
        }}
        Stored summaries are available in Results history when local storage is available.
      </p>
    </div>

    <div
      v-if="snapshot.dataset"
      class="space-y-2 rounded-md border p-4 text-sm"
    >
      <h3 class="font-medium">Dataset outcome</h3>
      <p>
        {{ snapshot.dataset.records }} records from {{ snapshot.dataset.source }}
        <template v-if="snapshot.dataset.version">
          (Faker {{ snapshot.dataset.version }}, {{ snapshot.dataset.locale }})
        </template>
        ; seed {{ snapshot.dataset.seed }}.
      </p>
      <p>
        Lifetime {{ snapshot.dataset.lifetime }}, allocation {{ snapshot.dataset.allocation }},
        exhaustion policy {{ snapshot.dataset.exhaustion }}.
      </p>
      <p>
        {{ snapshot.dataset.allocated }} allocations; {{ snapshot.dataset.cycles }} repeat cycles.
        {{
          snapshot.dataset.exhausted
            ? 'Dataset exhausted. Admission closed and in-flight requests drained.'
            : 'Dataset was not exhausted.'
        }}
      </p>
    </div>

    <RunMeasurement :snapshot="snapshot" />

    <p
      v-if="snapshot.failure_code"
      class="text-destructive"
    >
      {{ snapshot.failure_code }}
    </p>

    <h3 class="text-sm font-medium">Full-run accounting, including warm-up</h3>

    <dl class="grid gap-x-8 gap-y-4 text-sm sm:grid-cols-2">
      <div
        v-for="(value, name) in snapshot.metrics.counters"
        :key="name"
        class="flex justify-between gap-4 border-b pb-2"
      >
        <dt class="text-muted-foreground capitalize">{{ name.replaceAll('_', ' ') }}</dt>
        <dd class="font-mono">{{ value }}</dd>
      </div>

      <div
        v-for="(value, name) in {
          scheduling: snapshot.scheduling_us,
          drain: snapshot.drain_us,
          actual: snapshot.actual_us,
        }"
        :key="name"
        class="flex justify-between gap-4 border-b pb-2"
      >
        <dt class="text-muted-foreground capitalize">{{ name }} duration</dt>
        <dd class="font-mono">{{ (Number(value) / 1e6).toFixed(3) }} s</dd>
      </div>

      <div class="flex justify-between gap-4 border-b pb-2">
        <dt class="text-muted-foreground">Peak requested / actual VUs</dt>
        <dd class="font-mono">{{ snapshot.confirmation.vus }} / {{ snapshot.max_vus }}</dd>
      </div>

      <div class="flex justify-between gap-4 border-b pb-2">
        <dt class="text-muted-foreground">Evaluated HTTP failure rate</dt>

        <dd class="font-mono">
          {{
            snapshot.evaluated.http_failure_rate === null
              ? 'No data'
              : `${(snapshot.evaluated.http_failure_rate * 100).toFixed(2)}%`
          }}
        </dd>
      </div>

      <div
        v-for="(value, name) in {
          requests: snapshot.evaluated.request_rate,
          successes: snapshot.evaluated.success_rate,
          iterations: snapshot.evaluated.iteration_rate,
        }"
        :key="name"
        class="flex justify-between gap-4 border-b pb-2"
      >
        <dt class="text-muted-foreground capitalize">Evaluated {{ name }} per second</dt>
        <dd class="font-mono">{{ value.toFixed(2) }}</dd>
      </div>
    </dl>

    <div
      v-if="snapshot.thresholds.length"
      class="overflow-x-auto"
    >
      <table class="w-full text-left text-xs">
        <caption class="text-muted-foreground mb-3 text-left">
          Final thresholds excluding warm-up for
          {{ snapshot.measurement.complete ? 'completed' : 'partial' }} work. Comparisons use
          unrounded values. Insufficient data cannot pass.
        </caption>

        <thead>
          <tr>
            <th class="p-2">Metric / population</th>
            <th class="p-2">Required</th>
            <th class="p-2">Observed</th>
            <th class="p-2">Result</th>
          </tr>
        </thead>

        <tbody>
          <tr
            v-for="(result, index) in snapshot.thresholds"
            :key="index"
            class="border-t"
          >
            <th class="p-2 font-normal">
              {{
                result.rule.metric === 'response_p95_ms'
                  ? 'Completed response p95 (ms)'
                  : result.rule.metric === 'http_failure_percent'
                    ? 'HTTP failure (%) / terminal attempts'
                    : result.rule.metric === 'assertion_failure_percent'
                      ? 'Assertion failure (%) / evaluated checks'
                      : 'Terminal requests/s / evaluated duration'
              }}
            </th>

            <td class="p-2 font-mono">
              {{ result.rule.operator === 'less_than' ? '<' : '>' }} {{ result.rule.limit }}
            </td>

            <td class="p-2 font-mono">
              {{ result.observed === null ? 'No data' : result.observed.toFixed(3) }}
            </td>

            <td
              :class="result.status === 'passed' ? '' : 'text-destructive'"
              class="p-2 capitalize"
            >
              {{ result.status.replaceAll('_', ' ') }}
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <h3 class="text-sm font-medium">Full-run diagnostics, including warm-up</h3>

    <RunScenarioResults
      v-if="snapshot.scenarios"
      :report="snapshot.scenarios"
    />

    <RunResponseDiagnostics :diagnostics="snapshot.metrics.diagnostics" />

    <RunArrivalMetrics :snapshot="snapshot" />

    <div class="overflow-x-auto">
      <table class="w-full text-left text-xs">
        <caption class="text-muted-foreground mb-3 text-left leading-relaxed">
          Evaluated approximate HDR durations in milliseconds, three significant digits, range 1
          μs–1 hour. Response latency includes completed HTTP responses of all statuses. Transport
          and timeout durations are separate populations.
        </caption>

        <thead>
          <tr>
            <th class="p-2">Population</th>
            <th class="p-2">Samples</th>
            <th class="p-2">Outside range</th>

            <th
              v-for="name in ['mean', 'min', 'max', 'median', 'p90', 'p95', 'p99']"
              :key="name"
              class="p-2"
            >
              {{ name }}
            </th>
          </tr>
        </thead>

        <tbody>
          <tr
            v-for="(population, name) in {
              responses: snapshot.evaluated.response_latency,
              transport: snapshot.evaluated.transport_latency,
              timeouts: snapshot.evaluated.timeout_latency,
            }"
            :key="name"
            class="border-t"
          >
            <th class="p-2 font-normal capitalize">{{ name }}</th>
            <td class="p-2 font-mono">{{ population.samples }}</td>
            <td class="p-2 font-mono">{{ population.out_of_range }}</td>

            <td
              v-for="(value, key) in {
                mean: population.mean_us,
                min: population.min_us,
                max: population.max_us,
                median: population.median_us,
                p90: population.p90_us,
                p95: population.p95_us,
                p99: population.p99_us,
              }"
              :key="key"
              class="p-2 font-mono"
            >
              {{ value === null ? '—' : (value / 1000).toFixed(3) }}
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <p class="text-muted-foreground text-xs leading-relaxed">
      Body bytes are encoded response payload consumed, including partial failed responses. Headers,
      TLS, and retransmissions are excluded. HTTP failure rate divides status, transport, and
      timeout failures by non-cancelled terminal attempts. Evaluated throughput excludes warm-up
      time and includes drain. Aggregation wait: {{ snapshot.metrics.aggregation_wait_us }} μs;
      maximum in flight: {{ snapshot.metrics.max_in_flight }}.
    </p>
  </section>
</template>
