<script setup lang="ts">
  const { $runs } = useNuxtApp();
  const { snapshot, active } = $runs;
</script>

<template>
  <section
    v-if="snapshot && !active"
    class="mt-8 space-y-6 rounded-lg border p-6"
    aria-labelledby="summary-heading"
  >
    <div>
      <h2
        id="summary-heading"
        class="text-lg font-medium"
      >
        Final summary
      </h2>

      <p class="text-muted-foreground mt-2 text-sm">
        {{
          snapshot.state === 'completed'
            ? 'Scheduling completed.'
            : 'Partial run. Measurements cover observed work only.'
        }}
        Only the latest run is retained until another starts or the app exits.
      </p>
    </div>

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
        <dt class="text-muted-foreground">HTTP failure rate</dt>

        <dd class="font-mono">
          {{
            snapshot.metrics.http_failure_rate === null
              ? 'No data'
              : `${(snapshot.metrics.http_failure_rate * 100).toFixed(2)}%`
          }}
        </dd>
      </div>

      <div
        v-for="(value, name) in {
          requests: snapshot.metrics.request_rate,
          successes: snapshot.metrics.success_rate,
          iterations: snapshot.metrics.iteration_rate,
        }"
        :key="name"
        class="flex justify-between gap-4 border-b pb-2"
      >
        <dt class="text-muted-foreground capitalize">{{ name }} per second</dt>
        <dd class="font-mono">{{ value.toFixed(2) }}</dd>
      </div>
    </dl>

    <div class="overflow-x-auto">
      <table class="w-full text-left text-xs">
        <caption class="text-muted-foreground mb-3 text-left leading-relaxed">
          Approximate HDR durations in milliseconds, three significant digits, range 1 μs–1 hour.
          Response latency includes completed HTTP responses of all statuses. Transport and timeout
          durations are separate populations.
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
              responses: snapshot.metrics.response_latency,
              transport: snapshot.metrics.transport_latency,
              timeouts: snapshot.metrics.timeout_latency,
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
      timeout failures by non-cancelled terminal attempts. Throughput uses actual duration including
      drain. Aggregation wait: {{ snapshot.metrics.aggregation_wait_us }} μs; maximum in flight:
      {{ snapshot.metrics.max_in_flight }}.
    </p>
  </section>
</template>
