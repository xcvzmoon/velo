<script setup lang="ts">
  import type { ResponseDiagnostics } from '~/types/contracts';

  defineProps<{ diagnostics: ResponseDiagnostics }>();
</script>

<template>
  <section
    class="space-y-4 rounded-md border p-4"
    aria-label="Response assertions and failure diagnostics"
  >
    <h3 class="text-sm font-medium">Response assertions and failure diagnostics</h3>

    <p class="text-muted-foreground text-xs">
      Checks stay separate from HTTP outcomes. Per assertion, passed + failed + unavailable equals
      terminal attempts, including cancellations. Invalid JSON, unsupported encoding, unreadable
      bodies and truncation cannot pass. Multiple failed assertions count separately.
    </p>

    <div
      v-if="diagnostics.assertions.length"
      class="overflow-x-auto"
    >
      <table class="w-full text-left text-xs">
        <caption class="sr-only">Per-assertion outcomes</caption>
        <thead>
          <tr>
            <th class="p-2">Assertion</th>
            <th class="p-2">Passed</th>
            <th class="p-2">Failed</th>
            <th class="p-2">Unavailable / error</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="total in diagnostics.assertions"
            :key="total.id"
            class="border-t"
          >
            <th class="p-2 font-normal">{{ total.id }}</th>
            <td class="p-2 font-mono">{{ total.passed }}</td>
            <td class="p-2 font-mono">{{ total.failed }}</td>
            <td class="p-2 font-mono">{{ total.unavailable }}</td>
          </tr>
        </tbody>
      </table>
    </div>

    <div
      v-if="diagnostics.groups.length"
      class="max-h-80 overflow-auto"
    >
      <table class="w-full text-left text-xs">
        <caption class="sr-only">Grouped failure causes</caption>
        <thead>
          <tr>
            <th class="p-2">Assertion</th>
            <th class="p-2">Cause</th>
            <th class="p-2">HTTP</th>
            <th class="p-2">Count</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="(group, index) in diagnostics.groups"
            :key="index"
            class="border-t"
          >
            <th class="p-2 font-normal">
              {{
                group.example.assertion === null
                  ? 'Request'
                  : diagnostics.assertions[group.example.assertion]?.id
              }}
            </th>
            <td class="p-2">{{ group.example.cause.replaceAll('_', ' ') }}</td>
            <td class="p-2 font-mono">{{ group.example.status ?? 'No response' }}</td>
            <td class="p-2 font-mono">{{ group.count }}</td>
          </tr>
        </tbody>
      </table>
    </div>

    <p
      v-else
      class="text-muted-foreground text-xs"
    >
      No failure causes observed.
    </p>

    <details
      v-if="diagnostics.samples.length"
      class="text-xs"
    >
      <summary class="cursor-pointer">
        {{ diagnostics.samples.length }} redacted representative samples
      </summary>

      <ul class="mt-3 space-y-2">
        <li
          v-for="(sample, index) in diagnostics.samples"
          :key="index"
          class="rounded border p-2"
        >
          {{ sample.assertion === null ? 'Request' : diagnostics.assertions[sample.assertion]?.id }}
          · HTTP {{ sample.status ?? 'unavailable' }} · {{ sample.cause.replaceAll('_', ' ') }}.
          Response content withheld.
        </li>
      </ul>
    </details>

    <p class="text-muted-foreground text-xs leading-relaxed">
      Inspection cap: {{ diagnostics.limits.body_bytes }} bytes per worker. Truncated responses:
      {{ diagnostics.truncated_responses }}. Samples: {{ diagnostics.samples.length }} /
      {{ diagnostics.limits.sample_count }}; {{ diagnostics.retained_bytes }} /
      {{ diagnostics.limits.sample_bytes }} serialized bytes. Dropped representatives:
      {{ diagnostics.dropped_samples }}; ungrouped occurrences: {{ diagnostics.dropped_groups }}.
      Samples contain status and cause only, with response bodies, headers and expected values
      withheld before IPC or storage. Deleting a run removes its samples.
    </p>
  </section>
</template>
