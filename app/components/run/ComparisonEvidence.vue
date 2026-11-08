<script setup lang="ts">
  defineProps<{ report: ComparisonReport }>();
</script>

<template>
  <div class="space-y-5">
    <p class="text-sm">
      Baseline {{ report.baseline_id }} · regression verdict: {{ report.verdict }}
    </p>

    <p class="text-muted text-sm">
      Safe configuration identity: {{ report.candidate.definition_identity ?? 'unavailable' }}.
      Request paths and content are withheld. Review the target conditions and omitted request
      context before accepting a conclusion.
    </p>

    <ul
      v-if="report.limitations.length"
      class="list-inside list-disc text-sm"
      aria-label="Comparison limitations"
    >
      <li
        v-for="limitation in report.limitations"
        :key="limitation"
      >
        {{ limitation }}
      </li>
    </ul>

    <div class="space-y-2">
      <h4 class="text-sm font-medium">Configuration differences</h4>

      <p
        v-if="!report.differences.length"
        class="text-muted text-sm"
      >
        No differences in retained configuration. Omitted request content may differ.
      </p>

      <dl
        v-for="difference in report.differences"
        :key="difference.field"
        class="border-default rounded-md border p-3 text-sm"
      >
        <dt class="font-medium">{{ difference.field }}</dt>

        <dd class="mt-2 overflow-auto font-mono text-xs break-all whitespace-pre-wrap">
          Baseline: {{ JSON.stringify(difference.baseline) }}
          <br />
          Candidate: {{ JSON.stringify(difference.candidate) }}
        </dd>
      </dl>
    </div>

    <RunComparisonCharts :deltas="report.deltas" />

    <div class="overflow-auto">
      <UTable
        :data="report.deltas"
        :columns="[
          { id: 'metric', header: 'Metric' },
          { id: 'baseline', header: 'Baseline' },
          { id: 'candidate', header: 'Candidate' },
          { id: 'absolute', header: 'Absolute change' },
          { id: 'percentage', header: 'Relative %' },
        ]"
        :ui="{ td: 'text-xs font-mono', th: 'text-xs' }"
        tabindex="0"
        class="w-full"
      >
        <template #caption>
          <span class="text-muted mb-2 text-left">
            Evaluated measurements. Absolute change = candidate − baseline. Relative change = 100 ×
            change / baseline. Values retain measurement precision.
          </span>
        </template>

        <template #metric-cell="{ row }">
          <span>{{ row.original.metric }}</span>
        </template>

        <template #baseline-cell="{ row }">
          <span>{{ row.original.baseline ?? 'unavailable' }}</span>
        </template>

        <template #candidate-cell="{ row }">
          <span>{{ row.original.candidate ?? 'unavailable' }}</span>
        </template>

        <template #absolute-cell="{ row }">
          <span>{{ row.original.absolute ?? 'unavailable' }}</span>
        </template>

        <template #percentage-cell="{ row }">
          <span>{{ row.original.percentage ?? 'unavailable' }}</span>
        </template>
      </UTable>
    </div>
  </div>
</template>
