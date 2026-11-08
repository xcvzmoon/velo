<script setup lang="ts">
  import type { HistoryEntry } from '~/types/contracts';

  const columns = [
    {
      id: 'target',
      header: 'Target',
    },
    {
      id: 'state',
      header: 'State',
    },
    {
      id: 'latency',
      header: 'Response p95',
    },
    {
      id: 'created',
      header: 'Started',
    },
  ];

  defineProps<{ entries: HistoryEntry[]; loading: boolean; unavailable: boolean }>();
</script>

<template>
  <UCard
    :ui="{ header: 'bg-elevated/50 rounded-t-xl p-4 sm:px-5', body: 'p-0 sm:p-0' }"
    class="bg-default ring-default min-w-0 rounded-xl shadow-sm"
  >
    <template #header>
      <div class="flex items-center justify-between gap-3">
        <h2 class="text-highlighted text-sm font-semibold">Recent runs</h2>

        <UButton
          to="/results"
          label="View history"
          color="neutral"
          variant="ghost"
        />
      </div>
    </template>

    <div
      v-if="loading && !entries.length"
      class="space-y-3 p-5"
      role="status"
      aria-label="Loading recent runs"
    >
      <USkeleton
        v-for="index in 3"
        :key="index"
        class="h-10 w-full"
      />
    </div>

    <UTable
      v-else
      :data="entries"
      :columns="columns"
      :loading="loading"
      tabindex="0"
      class="w-full"
    >
      <template #caption><span class="sr-only">Five most recent stored runs</span></template>

      <template #target-cell="{ row }">
        <div class="min-w-0">
          <p class="text-highlighted text-sm">{{ row.original.snapshot.confirmation.origin }}</p>

          <p class="text-muted mt-1 text-xs">
            {{ row.original.method }} · {{ row.original.snapshot.confirmation.workload.kind
            }}{{ row.original.metadata.release ? ` · ${row.original.metadata.release}` : '' }}
          </p>
        </div>
      </template>

      <template #state-cell="{ row }">
        <div class="flex flex-wrap gap-1">
          <UBadge
            :label="row.original.interrupted ? 'interrupted' : row.original.snapshot.state"
            :color="
              row.original.interrupted
                ? 'warning'
                : row.original.snapshot.state === 'failed'
                  ? 'error'
                  : 'neutral'
            "
            variant="subtle"
            class="text-highlighted capitalize"
          />

          <UBadge
            v-if="row.original.incomplete"
            label="Sample gaps"
            class="text-highlighted"
            color="warning"
            variant="subtle"
          />
        </div>
      </template>

      <template #latency-cell="{ row }">
        <span class="font-mono text-sm tabular-nums">
          {{
            row.original.snapshot.metrics.response_latency.p95_us === null
              ? 'Unavailable'
              : `${(row.original.snapshot.metrics.response_latency.p95_us / 1000).toFixed(2)} ms`
          }}
        </span>
      </template>

      <template #created-cell="{ row }">
        <time
          :datetime="new Date(Number(row.original.created_at) * 1000).toISOString()"
          class="text-muted text-xs"
        >
          {{ new Date(Number(row.original.created_at) * 1000).toLocaleString() }}
        </time>
      </template>

      <template #empty>
        <UEmpty
          :title="unavailable ? 'Recent runs unavailable' : 'No stored runs yet'"
          :description="
            unavailable
              ? 'Refresh to try loading your local history again.'
              : 'Completed and interrupted runs will appear here after they are stored.'
          "
          :ui="{ root: 'py-8', title: 'text-sm', description: 'text-xs' }"
          variant="naked"
        />
      </template>
    </UTable>
  </UCard>
</template>
