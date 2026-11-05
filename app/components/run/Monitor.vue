<script setup lang="ts">
  const { $runs } = useNuxtApp();
  const { snapshot, points, active, busy, connected, gaps, error } = $runs;

  const seconds = computed<number[]>(() => points.value.map((point) => point.seconds));
  const requests = computed<(number | null)[]>(() => points.value.map((point) => point.requests));
  const latency = computed<(number | null)[]>(() => points.value.map((point) => point.latency));
  const failures = computed<(number | null)[]>(() => points.value.map((point) => point.failures));
  const vus = computed<(number | null)[]>(() => points.value.map((point) => point.vus));
  const progress = computed<number>(() =>
    snapshot.value
      ? Math.min(
          100,
          Number(snapshot.value.scheduling_us) / (snapshot.value.confirmation.duration_ms * 10),
        )
      : 0,
  );
</script>

<template>
  <section
    v-if="snapshot"
    class="space-y-6"
    aria-label="Current run"
  >
    <div class="flex flex-wrap items-center justify-between gap-4">
      <div>
        <p class="text-muted-foreground text-xs tracking-widest uppercase">
          Run {{ snapshot.run_id }}
        </p>

        <h2 class="mt-2 text-xl font-medium">{{ snapshot.confirmation.origin }}</h2>
      </div>

      <div class="flex items-center gap-3">
        <span
          class="rounded-md border px-3 py-1.5 font-mono text-xs capitalize"
          role="status"
        >
          {{ snapshot.state }}
        </span>

        <Button
          v-if="active"
          :disabled="busy || snapshot.state === 'stopping'"
          variant="destructive"
          @click="$runs.stop()"
        >
          {{ snapshot.state === 'stopping' ? 'Draining…' : 'Stop run' }}
        </Button>
      </div>
    </div>

    <div
      :aria-valuenow="Math.round(progress)"
      class="bg-muted h-1.5 overflow-hidden rounded-full"
      role="progressbar"
      aria-label="Scheduling progress"
      aria-valuemin="0"
      aria-valuemax="100"
    >
      <div
        :style="{ width: `${progress}%` }"
        class="bg-primary h-full"
      />
    </div>

    <dl class="grid grid-cols-2 gap-4 lg:grid-cols-4">
      <div class="rounded-lg border p-4">
        <dt class="text-muted-foreground text-xs">Requests started</dt>
        <dd class="mt-2 font-mono text-xl">{{ snapshot.metrics.counters.started }}</dd>
      </div>

      <div class="rounded-lg border p-4">
        <dt class="text-muted-foreground text-xs">Elapsed / scheduling target</dt>

        <dd class="mt-2 font-mono text-lg">
          {{ (Number(snapshot.actual_us) / 1e6).toFixed(1) }} /
          {{ snapshot.confirmation.duration_ms / 1000 }} s
        </dd>
      </div>

      <div class="rounded-lg border p-4">
        <dt class="text-muted-foreground text-xs">Active / maximum VUs</dt>
        <dd class="mt-2 font-mono text-xl">{{ snapshot.active_vus }} / {{ snapshot.max_vus }}</dd>
      </div>

      <div class="rounded-lg border p-4">
        <dt class="text-muted-foreground text-xs">In flight</dt>
        <dd class="mt-2 font-mono text-xl">{{ snapshot.metrics.counters.in_flight }}</dd>
      </div>
    </dl>

    <p
      v-if="error"
      class="text-destructive text-sm"
      role="alert"
    >
      {{ error }}
    </p>

    <p
      v-if="snapshot.failure_code"
      class="text-destructive text-sm"
      role="alert"
    >
      {{ snapshot.failure_code }}: The run failed. Measurements below include partial work.
    </p>

    <p
      v-if="
        snapshot.metrics.aggregation_interference ||
        snapshot.metrics.counter_overflow ||
        snapshot.metrics.response_latency.out_of_range !== '0'
      "
      class="text-destructive text-sm"
      role="status"
    >
      Measurement integrity warning. Aggregation interference:
      {{ snapshot.metrics.aggregation_interference }}; counter overflow:
      {{ snapshot.metrics.counter_overflow }}; response samples outside histogram range:
      {{ snapshot.metrics.response_latency.out_of_range }}.
    </p>

    <div class="grid gap-4 xl:grid-cols-2">
      <RunChart
        :seconds="seconds"
        :values="requests"
        title="Interval request rate"
        unit="req/s"
      />

      <RunChart
        :seconds="seconds"
        :values="latency"
        title="Interval response p95"
        unit="ms"
      />

      <RunChart
        :seconds="seconds"
        :values="failures"
        title="Interval HTTP failure rate"
        unit="%"
      />

      <RunChart
        :seconds="seconds"
        :values="vus"
        title="Active virtual users"
        unit="VUs"
      />
    </div>

    <p class="text-muted-foreground text-xs leading-relaxed">
      {{
        connected
          ? 'Live aggregate subscription connected.'
          : 'Live events unavailable; recovering native totals.'
      }}
      Charts retain the latest 600 points.
      {{ gaps ? 'Some intervals were missed; gaps are shown.' : 'No event gaps detected.' }}
      Empty response populations have no percentile. Request rates exclude cancellations.
    </p>
  </section>
</template>
