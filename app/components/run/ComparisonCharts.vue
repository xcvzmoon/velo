<script setup lang="ts">
  const props = defineProps<{ deltas: MetricDelta[] }>();
  const charts = computed<ReturnType<typeof comparisonChart>[]>(() =>
    props.deltas.map(comparisonChart),
  );
</script>

<template>
  <section
    class="space-y-3"
    aria-label="Baseline and candidate measurement charts"
  >
    <h4 class="text-sm font-medium">Baseline and candidate measurements</h4>

    <p class="text-muted text-sm">
      Each metric has its own zero-based scale. Bars show evaluated values, not percentage changes.
      Zero values have no visible bar; missing observations are unavailable. See the table for
      signed changes.
    </p>

    <div class="grid gap-4 xl:grid-cols-2">
      <figure
        v-for="chart in charts"
        :key="chart.metric"
        class="min-w-0 space-y-3 rounded-md border p-4"
      >
        <figcaption class="text-sm font-medium">{{ chart.label }}</figcaption>

        <div
          v-for="row in chart.rows"
          :key="row.label"
          class="space-y-1"
        >
          <div class="flex flex-wrap justify-between gap-2 text-xs">
            <span>{{ row.label }}</span>
            <span class="font-mono break-all">{{ row.value ?? 'unavailable' }}</span>
          </div>

          <div
            :aria-label="`${row.label}: ${row.value ?? 'unavailable'}; ${chart.label}`"
            class="bg-muted h-3 overflow-hidden rounded-sm"
            role="img"
          >
            <div
              v-if="row.width !== null"
              :style="{ width: `${row.width}%` }"
              :class="row.label === 'Baseline' ? 'bg-sky-500' : 'bg-amber-500'"
              class="h-full"
            />
          </div>
        </div>

        <div class="text-muted flex justify-between gap-2 text-xs">
          <span>0</span>
          <span class="break-all">{{ chart.maximum ?? 'No observations' }}</span>
        </div>
      </figure>
    </div>
  </section>
</template>
