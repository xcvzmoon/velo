<script setup lang="ts">
  const { $runs } = useNuxtApp();
  const { snapshot, points, active, busy, connected, gaps, error } = $runs;

  const seconds = computed<number[]>(() => points.value.map((point) => point.seconds));
  const requests = computed<(number | null)[]>(() => points.value.map((point) => point.requests));
  const iterations = computed<(number | null)[]>(() =>
    points.value.map((point) => point.iterations),
  );
  const latency = computed<(number | null)[]>(() => points.value.map((point) => point.latency));
  const failures = computed<(number | null)[]>(() => points.value.map((point) => point.failures));
  const assertionFailures = computed<(number | null)[]>(() =>
    points.value.map((point) => point.assertionFailures),
  );
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
    class="bg-elevated/30 space-y-6 rounded-xl border p-5"
    aria-label="Current run"
  >
    <div class="flex flex-wrap items-center justify-between gap-4">
      <div>
        <p class="text-muted text-xs tracking-widest uppercase">Run {{ snapshot.run_id }}</p>

        <h2 class="mt-2 text-xl font-medium break-all">{{ snapshot.confirmation.origin }}</h2>
      </div>

      <div class="flex items-center gap-3">
        <UBadge
          :color="active ? 'primary' : snapshot.state === 'failed' ? 'error' : 'neutral'"
          :label="snapshot.state"
          variant="subtle"
          class="capitalize"
          role="status"
        />

        <UButton
          v-if="active"
          :disabled="busy || snapshot.state === 'stopping'"
          :label="snapshot.state === 'stopping' ? 'Draining…' : 'Stop run'"
          color="error"
          @click="$runs.stop()"
        />
      </div>
    </div>

    <UProgress
      :model-value="progress"
      :max="100"
      :get-value-label="() => 'Scheduling progress'"
      size="xs"
    />

    <dl class="grid grid-cols-2 gap-4 lg:grid-cols-4">
      <div class="bg-muted/50 rounded-lg border p-4">
        <dt class="text-muted text-xs">Requests started</dt>
        <dd class="mt-2 font-mono text-xl break-all">{{ snapshot.metrics.counters.started }}</dd>
      </div>

      <div class="bg-muted/50 rounded-lg border p-4">
        <dt class="text-muted text-xs">Elapsed / scheduling target</dt>

        <dd class="mt-2 font-mono text-lg">
          {{ (Number(snapshot.actual_us) / 1e6).toFixed(1) }} /
          {{ snapshot.confirmation.duration_ms / 1000 }} s
        </dd>
      </div>

      <div class="bg-muted/50 rounded-lg border p-4">
        <dt class="text-muted text-xs">Desired / actual VUs</dt>

        <dd class="mt-2">
          <span class="font-mono text-xl">
            {{ snapshot.desired_vus }} / {{ snapshot.active_vus }}
          </span>

          <p class="text-muted mt-2 text-xs">
            {{ snapshot.retiring_vus }} retiring · {{ snapshot.max_vus }} peak actual
          </p>
        </dd>
      </div>

      <div class="bg-muted/50 rounded-lg border p-4">
        <dt class="text-muted text-xs">In flight</dt>
        <dd class="mt-2 font-mono text-xl break-all">{{ snapshot.metrics.counters.in_flight }}</dd>
      </div>
    </dl>

    <RunResponseDiagnostics
      v-if="active"
      :diagnostics="snapshot.metrics.diagnostics"
    />

    <RunMeasurement :snapshot="snapshot" />

    <RunGeneratorHealth
      :snapshot="snapshot"
      :points="points"
    />

    <p
      v-if="error"
      class="text-error text-sm"
      role="alert"
    >
      {{ error }}
    </p>

    <p
      v-if="snapshot.failure_code"
      class="text-error text-sm"
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
      class="text-error text-sm"
      role="status"
    >
      Measurement integrity warning. Aggregation interference:
      {{ snapshot.metrics.aggregation_interference }}; counter overflow:
      {{ snapshot.metrics.counter_overflow }}; response samples outside histogram range:
      {{ snapshot.metrics.response_latency.out_of_range }}.
    </p>

    <RunScenarioResults
      v-if="snapshot.scenarios"
      :report="snapshot.scenarios"
    />

    <div class="grid gap-4 xl:grid-cols-2">
      <RunChart
        :warmup-seconds="snapshot.measurement.warmup_ms / 1000"
        :automatic-stop-seconds="
          snapshot.automatic_stop ? Number(snapshot.automatic_stop.at_us) / 1e6 : undefined
        "
        :seconds="seconds"
        :values="requests"
        title="Interval request rate"
        unit="req/s"
      />

      <RunChart
        v-if="snapshot.scenarios"
        :warmup-seconds="snapshot.measurement.warmup_ms / 1000"
        :automatic-stop-seconds="
          snapshot.automatic_stop ? Number(snapshot.automatic_stop.at_us) / 1e6 : undefined
        "
        :seconds="seconds"
        :values="iterations"
        title="Cumulative scenario iteration rate"
        unit="iterations/s"
      />

      <RunChart
        :warmup-seconds="snapshot.measurement.warmup_ms / 1000"
        :automatic-stop-seconds="
          snapshot.automatic_stop ? Number(snapshot.automatic_stop.at_us) / 1e6 : undefined
        "
        :seconds="seconds"
        :values="latency"
        title="Interval response p95"
        unit="ms"
      />

      <RunChart
        :warmup-seconds="snapshot.measurement.warmup_ms / 1000"
        :automatic-stop-seconds="
          snapshot.automatic_stop ? Number(snapshot.automatic_stop.at_us) / 1e6 : undefined
        "
        :seconds="seconds"
        :values="failures"
        title="Interval HTTP failure rate"
        unit="%"
        percentage
      />

      <RunChart
        v-if="snapshot.scenarios || snapshot.metrics.diagnostics.assertions.length"
        :warmup-seconds="snapshot.measurement.warmup_ms / 1000"
        :automatic-stop-seconds="
          snapshot.automatic_stop ? Number(snapshot.automatic_stop.at_us) / 1e6 : undefined
        "
        :seconds="seconds"
        :values="assertionFailures"
        title="Cumulative assertion failure rate"
        unit="%"
        percentage
      />
    </div>

    <p class="text-muted text-xs leading-relaxed">
      {{
        connected
          ? 'Live aggregate subscription connected.'
          : 'Live events unavailable; recovering native totals.'
      }}
      Charts include warm-up traffic. Markers show measurement start and automatic stop. Assertion
      failure rate uses cumulative full-run checks. Unavailable checks or counter overflow leave a
      gap; HTTP failures remain separate. Charts retain the latest 600 points.
      {{ gaps ? 'Some intervals were missed; gaps are shown.' : 'No event gaps detected.' }}
      Actual VUs include retiring users; slow retirees may defer a later ramp-up within the worker
      budget. Empty response populations have no percentile. Request rates exclude cancellations.
    </p>
  </section>
</template>
