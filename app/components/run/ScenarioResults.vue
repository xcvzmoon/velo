<script setup lang="ts">
  import type { ScenarioReport } from '~/types/contracts';

  defineProps<{ report: ScenarioReport }>();
</script>

<template>
  <section
    class="space-y-4"
    aria-label="Scenario outcomes"
  >
    <h3 class="font-medium">Scenario outcomes</h3>

    <p class="text-sm">Setup: {{ report.setup }} · Cleanup: {{ report.cleanup }}</p>

    <p
      v-if="report.cleanup === 'failed' || report.cleanup === 'cancelled'"
      class="text-error text-sm"
      role="alert"
    >
      Cleanup did not finish successfully. Target records may remain.
    </p>

    <div class="overflow-x-auto">
      <UTable
        :data="report.iterations"
        :columns="[
          { id: 'scenario', header: 'Scenario' },
          { id: 'started', header: 'Started' },
          { id: 'completed', header: 'Completed' },
          { id: 'failed', header: 'Failed' },
          { id: 'cancelled', header: 'Cancelled' },
        ]"
        :ui="{ td: 'text-xs font-mono', th: 'text-xs' }"
        tabindex="0"
        class="w-full"
      >
        <template #caption>
          <span class="text-muted mb-3 text-left">
            Measured scenario iterations. A failed iteration can contain successful HTTP requests.
          </span>
        </template>

        <template #scenario-cell="{ row }">
          <span class="font-normal">{{ row.original.id }}</span>
        </template>

        <template #started-cell="{ row }">
          <span class="font-mono">{{ row.original.started }}</span>
        </template>

        <template #completed-cell="{ row }">
          <span class="font-mono">{{ row.original.completed }}</span>
        </template>

        <template #failed-cell="{ row }">
          <span class="font-mono">{{ row.original.failed }}</span>
        </template>

        <template #cancelled-cell="{ row }">
          <span class="font-mono">{{ row.original.cancelled }}</span>
        </template>
      </UTable>
    </div>

    <div class="overflow-x-auto">
      <UTable
        :data="report.steps"
        :columns="[
          { id: 'identity', header: 'Phase / scenario / step' },
          { id: 'requests', header: 'Requests' },
          { id: 'failed', header: 'Failed' },
          { id: 'cancelled', header: 'Cancelled' },
          { id: 'binding_failures', header: 'Binding failures' },
          { id: 'extraction_failures', header: 'Extraction failures' },
          { id: 'p95', header: 'Response p95 (ms)' },
          { id: 'last_error', header: 'Last error' },
        ]"
        :ui="{ td: 'text-xs font-mono', th: 'text-xs' }"
        tabindex="0"
        class="w-full"
      >
        <template #caption>
          <span class="text-muted mb-3 text-left">
            Stable configured step identities. Setup and cleanup are excluded from measured load.
            Think time is excluded from HTTP response latency.
          </span>
        </template>

        <template #identity-cell="{ row }">
          <span class="font-normal">
            {{ row.original.phase }} / {{ row.original.scenario_id || 'lifecycle' }} /
            {{ row.original.step_id }}
          </span>
        </template>

        <template #requests-cell="{ row }">
          <span class="font-mono">{{ row.original.requests }}</span>
        </template>

        <template #failed-cell="{ row }">
          <span class="font-mono">{{ row.original.failed }}</span>
        </template>

        <template #cancelled-cell="{ row }">
          <span class="font-mono">{{ row.original.cancelled }}</span>
        </template>

        <template #binding_failures-cell="{ row }">
          <span class="font-mono">{{ row.original.binding_failures }}</span>
        </template>

        <template #extraction_failures-cell="{ row }">
          <span class="font-mono">{{ row.original.extraction_failures }}</span>
        </template>

        <template #p95-cell="{ row }">
          <span class="font-mono">
            {{
              row.original.response_latency.p95_us === null
                ? 'No data'
                : (row.original.response_latency.p95_us / 1000).toFixed(3)
            }}
          </span>
        </template>

        <template #last_error-cell="{ row }">
          <span>{{ row.original.last_error ?? 'None' }}</span>
        </template>
      </UTable>
    </div>
  </section>
</template>
