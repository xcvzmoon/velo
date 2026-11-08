<script setup lang="ts">
  import type { RunSnapshot } from '~/types/contracts';

  const props = defineProps<{ storedSnapshot?: RunSnapshot }>();

  const { $runs } = useNuxtApp();
  const headingId = useId();
  const snapshot = computed<RunSnapshot | null>(() => props.storedSnapshot ?? $runs.snapshot.value);
  const active = computed<boolean>(() => (props.storedSnapshot ? false : $runs.active.value));
</script>

<template>
  <ShellPanel
    v-if="snapshot && !active"
    :aria-labelledby="headingId"
    variant="section"
  >
    <template #header>
      <div>
        <h2
          :id="headingId"
          class="text-highlighted text-lg font-semibold"
        >
          Final summary
        </h2>

        <p class="text-muted mt-2 text-sm">
          {{
            snapshot.measurement.complete
              ? 'Scheduling completed.'
              : 'Partial run. Measurements cover observed work only.'
          }}
          Stored summaries are available in Results history when local storage is available.
        </p>
      </div>
    </template>

    <div
      v-if="snapshot.dataset"
      class="border-default space-y-2 rounded-md border p-4 text-sm"
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
      class="text-error"
    >
      {{ snapshot.failure_code }}
    </p>

    <h3 class="text-sm font-medium">Full-run accounting, including warm-up</h3>

    <dl class="grid gap-x-8 gap-y-4 text-sm sm:grid-cols-2">
      <div
        v-for="(value, name) in snapshot.metrics.counters"
        :key="name"
        class="border-default flex justify-between gap-4 border-b pb-2"
      >
        <dt class="text-muted capitalize">{{ name.replaceAll('_', ' ') }}</dt>
        <dd class="font-mono">{{ value }}</dd>
      </div>

      <div
        v-for="(value, name) in {
          scheduling: snapshot.scheduling_us,
          drain: snapshot.drain_us,
          actual: snapshot.actual_us,
        }"
        :key="name"
        class="border-default flex justify-between gap-4 border-b pb-2"
      >
        <dt class="text-muted capitalize">{{ name }} duration</dt>
        <dd class="font-mono">{{ (Number(value) / 1e6).toFixed(3) }} s</dd>
      </div>

      <div class="border-default flex justify-between gap-4 border-b pb-2">
        <dt class="text-muted">Peak requested / actual VUs</dt>
        <dd class="font-mono">{{ snapshot.confirmation.vus }} / {{ snapshot.max_vus }}</dd>
      </div>

      <div class="border-default flex justify-between gap-4 border-b pb-2">
        <dt class="text-muted">Evaluated HTTP failure rate</dt>

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
        class="border-default flex justify-between gap-4 border-b pb-2"
      >
        <dt class="text-muted capitalize">Evaluated {{ name }} per second</dt>
        <dd class="font-mono">{{ value.toFixed(2) }}</dd>
      </div>
    </dl>

    <div
      v-if="snapshot.thresholds.length"
      class="overflow-x-auto"
    >
      <UTable
        :data="snapshot.thresholds"
        :columns="[
          { id: 'metric', header: 'Metric / population' },
          { id: 'required', header: 'Required' },
          { id: 'observed', header: 'Observed' },
          { id: 'status', header: 'Result' },
        ]"
        :ui="{ td: 'text-xs font-mono', th: 'text-xs' }"
        tabindex="0"
        class="w-full"
      >
        <template #caption>
          <span class="text-muted mb-3 text-left">
            Final thresholds excluding warm-up for
            {{ snapshot.measurement.complete ? 'completed' : 'partial' }} work. Comparisons use
            unrounded values. Insufficient data cannot pass.
          </span>
        </template>

        <template #metric-cell="{ row }">
          <span class="font-normal">
            {{
              row.original.rule.metric === 'response_p95_ms'
                ? 'Completed response p95 (ms)'
                : row.original.rule.metric === 'http_failure_percent'
                  ? 'HTTP failure (%) / terminal attempts'
                  : row.original.rule.metric === 'assertion_failure_percent'
                    ? 'Assertion failure (%) / evaluated checks'
                    : 'Terminal requests/s / evaluated duration'
            }}
          </span>
        </template>

        <template #required-cell="{ row }">
          <span class="font-mono">
            {{ row.original.rule.operator === 'less_than' ? '<' : '>' }}
            {{ row.original.rule.limit }}
          </span>
        </template>

        <template #observed-cell="{ row }">
          <span class="font-mono">
            {{ row.original.observed === null ? 'No data' : row.original.observed.toFixed(3) }}
          </span>
        </template>

        <template #status-cell="{ row }">
          <span
            :class="row.original.status === 'passed' ? '' : 'text-error'"
            class="capitalize"
          >
            {{ row.original.status.replaceAll('_', ' ') }}
          </span>
        </template>
      </UTable>
    </div>

    <h3 class="text-sm font-medium">Full-run diagnostics, including warm-up</h3>

    <RunScenarioResults
      v-if="snapshot.scenarios"
      :report="snapshot.scenarios"
    />

    <RunResponseDiagnostics :diagnostics="snapshot.metrics.diagnostics" />

    <RunArrivalMetrics :snapshot="snapshot" />

    <div class="overflow-x-auto">
      <UTable
        :data="[
          { name: 'responses', population: snapshot.evaluated.response_latency },
          { name: 'transport', population: snapshot.evaluated.transport_latency },
          { name: 'timeouts', population: snapshot.evaluated.timeout_latency },
        ]"
        :columns="[
          { id: 'name', header: 'Population' },
          { id: 'samples', header: 'Samples' },
          { id: 'out_of_range', header: 'Outside range' },
          { id: 'mean', header: 'mean' },
          { id: 'min', header: 'min' },
          { id: 'max', header: 'max' },
          { id: 'median', header: 'median' },
          { id: 'p90', header: 'p90' },
          { id: 'p95', header: 'p95' },
          { id: 'p99', header: 'p99' },
        ]"
        :ui="{ td: 'text-xs font-mono', th: 'text-xs' }"
        tabindex="0"
        class="w-full"
      >
        <template #caption>
          <span class="text-muted text-left text-xs leading-relaxed">
            Evaluated approximate HDR durations in milliseconds, three significant digits, range 1
            μs–1 hour. Response latency includes completed HTTP responses of all statuses. Transport
            and timeout durations are separate populations.
          </span>
        </template>

        <template #name-cell="{ row }">
          <span class="capitalize">{{ row.original.name }}</span>
        </template>

        <template #samples-cell="{ row }">{{ row.original.population.samples }}</template>
        <template #out_of_range-cell="{ row }">{{ row.original.population.out_of_range }}</template>

        <template #mean-cell="{ row }">
          {{
            row.original.population.mean_us === null
              ? 'Unavailable'
              : (row.original.population.mean_us / 1000).toFixed(3)
          }}
        </template>

        <template #min-cell="{ row }">
          {{
            row.original.population.min_us === null
              ? 'Unavailable'
              : (row.original.population.min_us / 1000).toFixed(3)
          }}
        </template>

        <template #max-cell="{ row }">
          {{
            row.original.population.max_us === null
              ? 'Unavailable'
              : (row.original.population.max_us / 1000).toFixed(3)
          }}
        </template>

        <template #median-cell="{ row }">
          {{
            row.original.population.median_us === null
              ? 'Unavailable'
              : (row.original.population.median_us / 1000).toFixed(3)
          }}
        </template>

        <template #p90-cell="{ row }">
          {{
            row.original.population.p90_us === null
              ? 'Unavailable'
              : (row.original.population.p90_us / 1000).toFixed(3)
          }}
        </template>

        <template #p95-cell="{ row }">
          {{
            row.original.population.p95_us === null
              ? 'Unavailable'
              : (row.original.population.p95_us / 1000).toFixed(3)
          }}
        </template>

        <template #p99-cell="{ row }">
          {{
            row.original.population.p99_us === null
              ? 'Unavailable'
              : (row.original.population.p99_us / 1000).toFixed(3)
          }}
        </template>
      </UTable>
    </div>

    <p class="text-muted text-xs leading-relaxed">
      Body bytes are encoded response payload consumed, including partial failed responses. Headers,
      TLS, and retransmissions are excluded. HTTP failure rate divides status, transport, and
      timeout failures by non-cancelled terminal attempts. Evaluated throughput excludes warm-up
      time and includes drain. Aggregation wait: {{ snapshot.metrics.aggregation_wait_us }} μs;
      maximum in flight: {{ snapshot.metrics.max_in_flight }}.
    </p>
  </ShellPanel>
</template>
