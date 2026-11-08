<script setup lang="ts">
  import type { RunSnapshot } from '~/types/contracts';
  import type { OverviewMetric } from '~/utils/overview';

  const metricStyles = [
    {
      card: 'bg-primary/5 ring-primary/25',
      icon: 'bg-primary/10 text-primary ring-primary/20',
    },
    {
      card: 'bg-info/5 ring-info/25',
      icon: 'bg-info/10 text-info ring-info/20',
    },
    {
      card: 'bg-warning/5 ring-warning/25',
      icon: 'bg-warning/10 text-warning ring-warning/20',
    },
    {
      card: 'bg-error/5 ring-error/25',
      icon: 'bg-error/10 text-error ring-error/20',
    },
  ];

  const props = defineProps<{ snapshot: RunSnapshot | null; loading: boolean }>();
  const metrics = computed<OverviewMetric[]>(() => overviewMetrics(props.snapshot));
</script>

<template>
  <section
    :aria-busy="loading"
    class="grid min-w-0 gap-3 sm:grid-cols-2 xl:grid-cols-4"
    aria-label="Run metrics"
  >
    <UCard
      v-for="(metric, index) in metrics"
      :key="metric.label"
      :ui="{ body: 'flex h-full flex-col p-4 sm:p-5' }"
      :class="metricStyles[index]?.card"
      class="min-w-0 rounded-xl shadow-sm"
    >
      <div class="flex items-center justify-between gap-3">
        <h2 class="text-muted text-sm font-medium">{{ metric.label }}</h2>

        <span
          :class="metricStyles[index]?.icon"
          class="flex size-9 shrink-0 items-center justify-center rounded-lg ring ring-inset"
        >
          <UIcon
            :name="metric.icon"
            class="size-4"
          />
        </span>
      </div>

      <USkeleton
        v-if="loading && !snapshot"
        class="mt-4 h-8 w-24"
      />

      <p
        v-else
        class="text-highlighted mt-3 font-mono text-2xl tracking-tight break-all tabular-nums"
      >
        {{ metric.value }}
        <span
          v-if="metric.unit"
          class="text-muted text-sm tracking-normal"
        >
          {{ metric.unit }}
        </span>
      </p>

      <p class="text-muted border-default/60 mt-4 border-t pt-3 text-xs">{{ metric.detail }}</p>
    </UCard>
  </section>
</template>
