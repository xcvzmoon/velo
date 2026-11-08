<script setup lang="ts">
  const props = defineProps<{
    snapshot: RunSnapshot;
    points: GeneratorPoint[];
    incomplete?: boolean;
    stride?: number;
  }>();

  const headingId = useId();
  const current = computed<HostTelemetry | null>(() =>
    isTelemetryAvailable(props.snapshot.telemetry, Number(props.snapshot.actual_us) / 1e6)
      ? props.snapshot.telemetry
      : null,
  );
  const rates = computed<GeneratorRequestRates>(() => generatorRequestRates(props.snapshot));
  const resourceSamples = computed<GeneratorResourcePoint[]>(() =>
    generatorResourcePoints(props.points),
  );
  const resourceSeconds = computed<number[]>(() =>
    resourceSamples.value.map(({ seconds }) => seconds),
  );
  const processCpu = computed<(number | null)[]>(() =>
    resourceSamples.value.map(({ telemetry }) => telemetry?.process_cpu_percent ?? null),
  );
  const hostCpu = computed<(number | null)[]>(() =>
    resourceSamples.value.map(({ telemetry }) => telemetry?.system_cpu_percent ?? null),
  );
  const memory = computed<(number | null)[]>(() =>
    resourceSamples.value.map(({ telemetry }) =>
      telemetry?.process_memory_bytes ? Number(telemetry.process_memory_bytes) / 1_048_576 : null,
    ),
  );
  const availableMemory = computed<(number | null)[]>(() =>
    resourceSamples.value.map(({ telemetry }) =>
      telemetry?.system_available_memory_bytes === null ||
      telemetry?.system_available_memory_bytes === undefined
        ? null
        : Number(telemetry.system_available_memory_bytes) / 1_048_576,
    ),
  );
  const observations = computed<GeneratorObservation[]>(() => generatorObservations(props.points));
  const evidenceRows = computed<GeneratorObservation[]>(() => {
    const result: GeneratorObservation[] = [];
    for (const observation of observations.value) {
      if (observation.evidence.length > 0) {
        result.push(observation);
      }
    }
    return result.slice(-20);
  });
  const observationSeconds = computed<number[]>(() =>
    observations.value.map(({ seconds }) => seconds),
  );
  const pressure = computed<(number | null)[]>(() =>
    observations.value.map((item) => item.pressure),
  );
  const retiring = computed<(number | null)[]>(() =>
    props.points.map(({ retiringVus }) => retiringVus),
  );
  const vus = computed<(number | null)[]>(() => props.points.map((point) => point.vus));
  const desired = computed<(number | null)[]>(() =>
    props.points.map(({ desiredVus }) => desiredVus),
  );
  const workloadSeconds = computed<number[]>(() => props.points.map(({ seconds }) => seconds));

  function percent(value: number | null | undefined): string {
    return value === null || value === undefined ? 'Unavailable' : `${value.toFixed(1)}%`;
  }

  function mebibytes(bytes: string | null | undefined): string {
    return bytes === null || bytes === undefined
      ? 'Unavailable'
      : `${(Number(bytes) / 1_048_576).toFixed(1)} MiB`;
  }
</script>

<template>
  <section
    :aria-labelledby="headingId"
    class="@container space-y-4 rounded-lg border p-4"
  >
    <h3
      :id="headingId"
      class="text-sm font-medium"
    >
      Generator health and capacity
    </h3>

    <dl class="grid gap-4 text-sm @min-[20rem]:grid-cols-2 @min-[50rem]:grid-cols-4">
      <div>
        <dt class="text-muted-foreground">Native process CPU / resident memory</dt>
        <dd class="mt-1 font-mono">
          {{ percent(current?.process_cpu_percent) }} /
          {{ mebibytes(current?.process_memory_bytes) }}
        </dd>
      </div>

      <div>
        <dt class="text-muted-foreground">Host CPU / available RAM</dt>
        <dd class="mt-1 font-mono">
          {{ percent(current?.system_cpu_percent) }} /
          {{ mebibytes(current?.system_available_memory_bytes) }}
        </dd>
      </div>

      <div>
        <dt class="text-muted-foreground">Started / completed HTTP requests per second</dt>
        <dd class="mt-1 font-mono">
          {{ rates.started?.toFixed(2) ?? 'Unavailable' }} /
          {{ rates.completed?.toFixed(2) ?? 'Unavailable' }}
        </dd>
      </div>

      <div>
        <dt class="text-muted-foreground">Desired / actual / retiring VUs</dt>
        <dd class="mt-1 font-mono">
          {{ snapshot.desired_vus }} / {{ snapshot.active_vus }} / {{ snapshot.retiring_vus }}
        </dd>
      </div>
    </dl>

    <RunArrivalMetrics :snapshot="snapshot" />

    <details class="text-muted-foreground text-xs">
      <summary class="cursor-pointer">Measurement scope and rate windows</summary>

      <p class="text-muted-foreground text-xs leading-relaxed">
        Native sampling targets 1 s, independently of display updates. CPU is normalized to all
        {{ current?.logical_cpus ?? 'available' }} logical CPUs: 100% means total machine CPU
        capacity. Native process scope excludes WebView subprocesses. Resident memory and host
        available RAM use MiB (1,048,576 bytes); available RAM follows OS accounting, not a measured
        memory-stall metric. Native state: {{ snapshot.telemetry?.status ?? 'unavailable' }}.
        <template v-if="current">
          Last sample at {{ (Number(current.sampled_at_us) / 1e6).toFixed(2) }} s over
          {{ (Number(current.interval_us) / 1e6).toFixed(2) }} s.
        </template>
        Samples more than 2.5 s old are unavailable. Short runs may finish before a CPU window
        exists. Started and completed HTTP rates share the elapsed scheduling-plus-drain window;
        completions include HTTP, transport and timeout outcomes, and exclude cancellations.
      </p>
    </details>

    <p
      v-if="incomplete"
      class="text-muted-foreground text-xs"
      role="status"
    >
      History has missing intervals. Latest resource observations remain visible; timeline charts
      are hidden.
    </p>

    <div
      v-else-if="points.length"
      class="grid gap-4 @min-[48rem]:grid-cols-2"
    >
      <RunChart
        :warmup-seconds="snapshot.measurement.warmup_ms / 1000"
        :automatic-stop-seconds="
          snapshot.automatic_stop ? Number(snapshot.automatic_stop.at_us) / 1e6 : undefined
        "
        :seconds="resourceSeconds"
        :values="processCpu"
        title="Native process CPU"
        unit="% of machine"
        percentage
      />

      <RunChart
        :warmup-seconds="snapshot.measurement.warmup_ms / 1000"
        :automatic-stop-seconds="
          snapshot.automatic_stop ? Number(snapshot.automatic_stop.at_us) / 1e6 : undefined
        "
        :seconds="resourceSeconds"
        :values="hostCpu"
        title="Host CPU"
        unit="% of machine"
        percentage
      />

      <RunChart
        :warmup-seconds="snapshot.measurement.warmup_ms / 1000"
        :automatic-stop-seconds="
          snapshot.automatic_stop ? Number(snapshot.automatic_stop.at_us) / 1e6 : undefined
        "
        :seconds="resourceSeconds"
        :values="memory"
        title="Native process resident memory"
        unit="MiB"
      />

      <RunChart
        :warmup-seconds="snapshot.measurement.warmup_ms / 1000"
        :automatic-stop-seconds="
          snapshot.automatic_stop ? Number(snapshot.automatic_stop.at_us) / 1e6 : undefined
        "
        :seconds="resourceSeconds"
        :values="availableMemory"
        title="Host available RAM"
        unit="MiB"
      />

      <RunChart
        :warmup-seconds="snapshot.measurement.warmup_ms / 1000"
        :automatic-stop-seconds="
          snapshot.automatic_stop ? Number(snapshot.automatic_stop.at_us) / 1e6 : undefined
        "
        :seconds="observationSeconds"
        :values="pressure"
        title="Observed generator pressure (1 = evidence)"
        unit="evidence"
        binary
      />

      <RunChart
        :warmup-seconds="snapshot.measurement.warmup_ms / 1000"
        :automatic-stop-seconds="
          snapshot.automatic_stop ? Number(snapshot.automatic_stop.at_us) / 1e6 : undefined
        "
        :seconds="workloadSeconds"
        :values="vus"
        :comparison-values="desired"
        title="Desired and actual virtual users"
        unit="VUs"
      />

      <RunChart
        :warmup-seconds="snapshot.measurement.warmup_ms / 1000"
        :automatic-stop-seconds="
          snapshot.automatic_stop ? Number(snapshot.automatic_stop.at_us) / 1e6 : undefined
        "
        :seconds="workloadSeconds"
        :values="retiring"
        title="Retiring virtual users"
        unit="VUs"
      />
    </div>

    <details class="text-muted-foreground text-xs">
      <summary class="cursor-pointer">Pressure indicators, gaps and retention</summary>

      <p class="text-muted-foreground text-xs leading-relaxed">
        Pressure markers cite CPU ≥90% of machine capacity, available host RAM ≤10%, or observed
        admission drops and missed slots. These are indicators, not throughput limits or a diagnosis
        of the target service, database or network. No listed evidence does not prove spare
        capacity. Missing resource data never means healthy capacity. HTTP and assertion failures
        remain separate. Timelines use workload-relative seconds, including warm-up and drain.
        Native charts use the native sample timestamps; repeated samples are shown once. Gaps over
        2.5 s are not connected. Retention is bounded to 600 aggregate samples; native samples
        between display updates are not retained.
        {{
          stride && stride > 1
            ? `History is downsampled to every ${stride} aggregate intervals; omitted peaks cannot be reconstructed.`
            : ''
        }}
      </p>
    </details>

    <details
      v-if="!incomplete && evidenceRows.length"
      class="text-xs"
    >
      <summary class="cursor-pointer">
        Latest retained interval evidence (up to 20 observations)
      </summary>

      <ul class="mt-3 space-y-2 font-mono">
        <li
          v-for="(row, index) in evidenceRows"
          :key="index"
        >
          {{ row.seconds.toFixed(2) }} s: {{ row.evidence.join('; ') }}
        </li>
      </ul>
    </details>
  </section>
</template>
