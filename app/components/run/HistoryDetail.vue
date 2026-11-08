<script setup lang="ts">
  const props = defineProps<{ entry: HistoryEntry }>();

  const series = computed<ReturnType<typeof storedHistorySeries>>(() =>
    storedHistorySeries(props.entry.points),
  );
  const generatorPoints = computed<GeneratorPoint[]>(() =>
    storedGeneratorPoints(props.entry.points),
  );
</script>

<template>
  <article
    :aria-label="`Stored run ${entry.id}`"
    class="min-w-0 space-y-4"
  >
    <h3 class="text-sm font-medium">
      {{ new Date(Number(entry.created_at) * 1000).toLocaleString() }}
    </h3>

    <p class="text-muted text-sm">
      {{ entry.interrupted ? 'Interrupted; last observed measurements' : entry.snapshot.state }} ·
      {{ entry.incomplete ? 'Missing history intervals' : 'No reported write gaps' }} ·
      {{
        entry.stride === 1
          ? 'Original aggregate resolution'
          : `Compacted: every ${entry.stride} aggregate intervals; gaps are not interpolated`
      }}
    </p>

    <RunSummary :stored-snapshot="entry.snapshot" />

    <AiAssistant
      :evidence="selectAiEvidence(entry, null)"
      :disabled="false"
      task="explain"
    />

    <RunGeneratorHealth
      :snapshot="entry.snapshot"
      :points="generatorPoints"
      :incomplete="entry.incomplete"
      :stride="entry.stride"
    />

    <p
      v-if="entry.incomplete"
      class="text-muted text-sm"
    >
      Charts are hidden because history contains missing intervals. Cumulative observations remain
      shown.
    </p>

    <div
      v-else-if="entry.points.length"
      class="space-y-4"
    >
      <RunChart
        :warmup-seconds="entry.snapshot.measurement.warmup_ms / 1000"
        :automatic-stop-seconds="
          entry.snapshot.automatic_stop
            ? Number(entry.snapshot.automatic_stop.at_us) / 1e6
            : undefined
        "
        :seconds="series.seconds"
        :values="series.requests"
        title="Stored request rate"
        unit="requests/s"
      />

      <RunChart
        v-if="entry.snapshot.scenarios"
        :warmup-seconds="entry.snapshot.measurement.warmup_ms / 1000"
        :automatic-stop-seconds="
          entry.snapshot.automatic_stop
            ? Number(entry.snapshot.automatic_stop.at_us) / 1e6
            : undefined
        "
        :seconds="series.seconds"
        :values="series.iterations"
        title="Stored cumulative scenario iteration rate"
        unit="iterations/s"
      />

      <RunChart
        v-if="entry.snapshot.scenarios || entry.snapshot.metrics.diagnostics.assertions.length"
        :warmup-seconds="entry.snapshot.measurement.warmup_ms / 1000"
        :automatic-stop-seconds="
          entry.snapshot.automatic_stop
            ? Number(entry.snapshot.automatic_stop.at_us) / 1e6
            : undefined
        "
        :seconds="series.seconds"
        :values="series.assertionFailures"
        title="Stored cumulative assertion failure rate"
        unit="%"
        percentage
      />
    </div>
  </article>
</template>
