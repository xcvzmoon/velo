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
      class="text-destructive text-sm"
      role="alert"
    >
      Cleanup did not finish successfully. Target records may remain.
    </p>

    <div class="overflow-x-auto">
      <table class="w-full text-left text-xs">
        <caption class="text-muted-foreground mb-3 text-left">
          Measured scenario iterations. A failed iteration can contain successful HTTP requests.
        </caption>

        <thead>
          <tr>
            <th class="p-2">Scenario</th>
            <th class="p-2">Started</th>
            <th class="p-2">Completed</th>
            <th class="p-2">Failed</th>
            <th class="p-2">Cancelled</th>
          </tr>
        </thead>

        <tbody>
          <tr
            v-for="scenario in report.iterations"
            :key="scenario.id"
            class="border-t"
          >
            <th class="p-2 font-normal">{{ scenario.id }}</th>
            <td class="p-2 font-mono">{{ scenario.started }}</td>
            <td class="p-2 font-mono">{{ scenario.completed }}</td>
            <td class="p-2 font-mono">{{ scenario.failed }}</td>
            <td class="p-2 font-mono">{{ scenario.cancelled }}</td>
          </tr>
        </tbody>
      </table>
    </div>

    <div class="overflow-x-auto">
      <table class="w-full text-left text-xs">
        <caption class="text-muted-foreground mb-3 text-left">
          Stable configured step identities. Setup and cleanup are excluded from measured load.
          Think time is excluded from HTTP response latency.
        </caption>

        <thead>
          <tr>
            <th class="p-2">Phase / scenario / step</th>
            <th class="p-2">Requests</th>
            <th class="p-2">Failed</th>
            <th class="p-2">Cancelled</th>
            <th class="p-2">Binding failures</th>
            <th class="p-2">Extraction failures</th>
            <th class="p-2">Response p95 (ms)</th>
            <th class="p-2">Last error</th>
          </tr>
        </thead>

        <tbody>
          <tr
            v-for="step in report.steps"
            :key="`${step.phase}-${step.scenario_id}-${step.step_id}`"
            class="border-t"
          >
            <th class="p-2 font-normal">
              {{ step.phase }} / {{ step.scenario_id || 'lifecycle' }} / {{ step.step_id }}
            </th>
            <td class="p-2 font-mono">{{ step.requests }}</td>
            <td class="p-2 font-mono">{{ step.failed }}</td>
            <td class="p-2 font-mono">{{ step.cancelled }}</td>
            <td class="p-2 font-mono">{{ step.binding_failures }}</td>
            <td class="p-2 font-mono">{{ step.extraction_failures }}</td>
            <td class="p-2 font-mono">
              {{
                step.response_latency.p95_us === null
                  ? 'No data'
                  : (step.response_latency.p95_us / 1000).toFixed(3)
              }}
            </td>
            <td class="p-2">{{ step.last_error ?? 'None' }}</td>
          </tr>
        </tbody>
      </table>
    </div>
  </section>
</template>
