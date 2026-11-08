<script setup lang="ts">
  import type { RunSnapshot } from '~/types/contracts';

  defineProps<{ snapshot: RunSnapshot }>();
</script>

<template>
  <div
    class="space-y-2 rounded-md border p-4 text-sm"
    role="status"
  >
    <p>
      Warm-up ends at {{ snapshot.measurement.warmup_ms / 1000 }} s. Evaluated time:
      {{ (Number(snapshot.measurement.evaluated_us) / 1e6).toFixed(3) }} s.
      {{
        snapshot.measurement.complete
          ? 'Measurement window complete.'
          : 'Measurement window incomplete.'
      }}
    </p>

    <p>
      Evaluated requests: {{ snapshot.evaluated.counters.started }} started,
      {{ snapshot.evaluated.counters.responses }} responses,
      {{ snapshot.evaluated.counters.cancelled }} cancelled,
      {{ snapshot.evaluated.counters.in_flight }} in flight.
    </p>

    <div
      v-if="snapshot.automatic_stop"
      class="text-destructive space-y-1"
    >
      <p>
        Automatic stop at {{ (Number(snapshot.automatic_stop.at_us) / 1e6).toFixed(3) }} s. Partial
        results retained.
      </p>

      <p>
        {{ snapshot.automatic_stop.policy.rule.metric }} required
        {{ snapshot.automatic_stop.policy.rule.operator === 'less_than' ? '<' : '>' }}
        {{ snapshot.automatic_stop.policy.rule.limit }}; observed
        {{ snapshot.automatic_stop.observed ?? 'unavailable' }} from
        {{ snapshot.automatic_stop.samples }} samples.
      </p>

      <p>
        Window {{ snapshot.automatic_stop.policy.window_ms }} ms; breach
        {{ (Number(snapshot.automatic_stop.breached_us) / 1000).toFixed(0) }} ms; required
        {{ snapshot.automatic_stop.policy.sustained_ms }} ms.
      </p>
    </div>
  </div>
</template>
