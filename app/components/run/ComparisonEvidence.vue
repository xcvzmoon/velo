<script setup lang="ts">
  defineProps<{ report: ComparisonReport }>();
</script>

<template>
  <div class="space-y-5">
    <p class="text-sm">
      Baseline {{ report.baseline_id }} · regression verdict: {{ report.verdict }}
    </p>

    <p class="text-muted-foreground text-sm">
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
        class="text-muted-foreground text-sm"
      >
        No differences in retained configuration. Omitted request content may differ.
      </p>

      <dl
        v-for="difference in report.differences"
        :key="difference.field"
        class="rounded-md border p-3 text-sm"
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
      <table class="w-full text-left text-sm">
        <caption class="text-muted-foreground mb-2 text-left">
          Evaluated measurements. Absolute change = candidate − baseline. Relative change = 100 ×
          change / baseline. Values retain measurement precision.
        </caption>
        <thead>
          <tr>
            <th class="p-2">Metric</th>
            <th class="p-2">Baseline</th>
            <th class="p-2">Candidate</th>
            <th class="p-2">Absolute change</th>
            <th class="p-2">Relative %</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="delta in report.deltas"
            :key="delta.metric"
            class="border-t"
          >
            <th class="p-2">{{ delta.metric }}</th>
            <td class="p-2">{{ delta.baseline ?? 'unavailable' }}</td>
            <td class="p-2">{{ delta.candidate ?? 'unavailable' }}</td>
            <td class="p-2">{{ delta.absolute ?? 'unavailable' }}</td>
            <td class="p-2">{{ delta.percentage ?? 'unavailable' }}</td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>
