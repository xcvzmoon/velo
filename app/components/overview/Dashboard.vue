<script setup lang="ts">
  import type { HistoryEntry, RunSnapshot } from '~/types/contracts';
  import type { OverviewTimeline } from '~/utils/overview';
  import { invoke } from '@tauri-apps/api/core';

  const actions = [
    {
      label: 'Configure a test',
      to: '/tests',
      icon: 'i-hugeicons-test-tube-01',
    },
    {
      label: 'Open API workspace',
      to: '/api',
      icon: 'i-hugeicons-code',
    },
    {
      label: 'Browse results',
      to: '/results',
      icon: 'i-hugeicons-chart-line-data-01',
    },
  ];

  const { $runs } = useNuxtApp();
  const { snapshot, points, active, busy, connected, gaps, error } = $runs;
  const history = shallowRef<HistoryEntry[]>([]);
  const loading = ref<boolean>(false);
  const historyError = ref<string | null>(null);
  const historyStatus = ref<string | null>(null);
  const recent = computed<HistoryEntry[]>(() => history.value.slice(0, 5));
  const displayed = computed<RunSnapshot | null>(
    () => snapshot.value ?? recent.value[0]?.snapshot ?? null,
  );
  const stored = computed<boolean>(() => !snapshot.value && displayed.value !== null);
  const interrupted = computed<boolean>(
    () => stored.value && recent.value[0]?.interrupted === true,
  );
  const scope = computed<string>(() =>
    active.value
      ? 'Current run'
      : stored.value
        ? 'Latest stored run'
        : displayed.value
          ? 'Latest run'
          : 'Run summary',
  );
  const timeline = computed<OverviewTimeline | null>(() => {
    if (stored.value) {
      return null;
    }
    const series: OverviewTimeline = {
      seconds: [],
      requests: [],
      latency: [],
    };
    for (const point of points.value) {
      series.seconds.push(point.seconds);
      series.requests.push(point.requests);
      series.latency.push(point.latency);
    }
    return series;
  });
  const limitations = computed<string[]>(() => {
    const current = displayed.value;
    if (!current) {
      return [];
    }
    const notes: string[] = [];
    if (interrupted.value) {
      notes.push('The application closed before this run finished. Measurements are partial.');
    } else if (!active.value && !current.measurement.complete) {
      notes.push('This run has an incomplete measurement window. Values cover observed work only.');
    }
    if (
      current.metrics.counter_overflow ||
      current.metrics.aggregation_interference ||
      current.metrics.response_latency.out_of_range !== '0'
    ) {
      notes.push(
        'Measurement integrity is limited. Review the diagnostics before interpreting these values.',
      );
    }
    if (current.failure_code) {
      notes.push(`Run failure: ${current.failure_code}.`);
    }
    return notes;
  });

  async function refresh(): Promise<void> {
    if (loading.value) {
      return;
    }
    loading.value = true;
    historyError.value = null;
    const [entries, status] = await Promise.allSettled([
      invoke<HistoryEntry[]>('list_history', { before: null }),
      invoke<string | null>('history_status'),
    ]);
    if (entries.status === 'fulfilled') {
      history.value = entries.value;
    } else {
      historyError.value =
        'HISTORY_UNAVAILABLE: Recent runs could not be refreshed. Displayed history may be stale.';
    }
    historyStatus.value =
      status.status === 'fulfilled' ? status.value : 'HISTORY_STATUS_UNAVAILABLE';
    loading.value = false;
  }

  watch(active, (isActive, wasActive) => {
    if (wasActive && !isActive) {
      void refresh();
    }
  });
  onMounted(refresh);
</script>

<template>
  <section
    class="mx-auto max-w-7xl min-w-0 space-y-6"
    aria-label="Overview dashboard"
  >
    <header class="flex flex-wrap items-start justify-between gap-4">
      <div>
        <h1 class="text-highlighted text-2xl font-semibold tracking-tight">Overview</h1>

        <p class="text-muted mt-1 text-sm">
          Monitor API performance and return to your latest work.
        </p>
      </div>

      <div class="flex flex-wrap gap-2">
        <UButton
          :loading="loading"
          :disabled="loading"
          label="Refresh"
          icon="i-hugeicons-refresh"
          color="neutral"
          variant="outline"
          @click="refresh"
        />

        <UButton
          to="/tests"
          label="Configure a test"
          icon="i-hugeicons-test-tube-01"
          color="neutral"
        />
      </div>
    </header>

    <UAlert
      :ui="{ title: 'text-highlighted', description: 'text-default' }"
      v-if="error"
      :description="error"
      title="Native run connection"
      color="warning"
      variant="subtle"
    />

    <UAlert
      :ui="{ title: 'text-highlighted', description: 'text-default' }"
      v-if="historyError || historyStatus"
      :title="historyError ? 'History unavailable' : 'History may be incomplete'"
      :description="historyError ?? `${historyStatus}: Some run evidence may not have been stored.`"
      color="warning"
      variant="subtle"
    />

    <section
      class="space-y-3"
      aria-label="Performance summary"
    >
      <div class="flex flex-wrap items-center justify-between gap-2">
        <h2 class="text-highlighted text-sm font-medium">{{ scope }}</h2>
        <p class="text-muted text-xs">Full-run measurements, including warm-up</p>
      </div>

      <OverviewMetrics
        :snapshot="displayed"
        :loading="loading"
      />

      <UAlert
        :ui="{ title: 'text-highlighted', description: 'text-default' }"
        v-if="limitations.length"
        :description="limitations.join(' ')"
        title="Measurement notes"
        color="warning"
        variant="subtle"
      />
    </section>

    <div class="grid min-w-0 items-start gap-4 xl:grid-cols-[minmax(0,1fr)_16rem]">
      <OverviewActivity
        :snapshot="displayed"
        :timeline="timeline"
        :active="active"
        :busy="busy"
        :connected="connected"
        :gaps="gaps"
        :stored="stored"
        :interrupted="interrupted"
        @stop="$runs.stop()"
      />

      <UCard
        :ui="{ header: 'bg-elevated/50 rounded-t-xl p-4', body: 'p-3 sm:p-3' }"
        class="bg-default ring-default min-w-0 rounded-xl shadow-sm"
      >
        <template #header>
          <h2 class="text-highlighted text-sm font-semibold">Quick actions</h2>
        </template>

        <ul class="space-y-1">
          <li
            v-for="action in actions"
            :key="action.to"
          >
            <UButton
              :to="action.to"
              :label="action.label"
              :icon="action.icon"
              color="neutral"
              variant="ghost"
              class="w-full justify-start"
            />
          </li>
        </ul>

        <p class="text-muted border-default mt-4 border-t px-2 pt-4 text-xs leading-relaxed">
          Review the target and workload before authorizing traffic.
        </p>
      </UCard>
    </div>

    <OverviewRecentRuns
      :entries="recent"
      :loading="loading"
      :unavailable="historyError !== null"
    />
  </section>
</template>
