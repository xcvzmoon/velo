<script setup lang="ts">
  import type { HistoryEntry } from '~/types/contracts';

  const props = defineProps<{ entry: HistoryEntry }>();

  const seconds = computed<number[]>(() =>
    props.entry.points.map(({ snapshot }) => Number(snapshot.actual_us) / 1_000_000),
  );
  const rates = computed<(number | null)[]>(() =>
    props.entry.points.map(({ interval_us, interval_requests }) =>
      Number(interval_us) > 0
        ? (Number(interval_requests) * 1_000_000) / Number(interval_us)
        : null,
    ),
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

    <p class="text-muted-foreground text-sm">
      {{ entry.interrupted ? 'Interrupted; last observed measurements' : entry.snapshot.state }} ·
      {{ entry.incomplete ? 'Missing history intervals' : 'No reported write gaps' }} ·
      {{
        entry.stride === 1
          ? 'Original aggregate resolution'
          : `Compacted: every ${entry.stride} aggregate intervals`
      }}
    </p>

    <RunSummary :stored-snapshot="entry.snapshot" />

    <p
      v-if="entry.incomplete"
      class="text-muted-foreground text-sm"
    >
      Charts are hidden because history contains missing intervals. Cumulative observations remain
      shown.
    </p>

    <RunChart
      v-else-if="entry.points.length"
      :seconds="seconds"
      :values="rates"
      title="Stored request rate"
      unit="requests/s"
    />
  </article>
</template>
