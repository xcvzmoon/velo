<script setup lang="ts">
  import type { RunSnapshot } from '~/types/contracts';

  defineProps<{ snapshot: RunSnapshot }>();
</script>

<template>
  <div
    v-if="snapshot.confirmation.workload.kind === 'arrival'"
    class="space-y-3 rounded-lg border p-4 text-sm"
  >
    <p>
      Target {{ snapshot.confirmation.workload.rate }} iterations/s · capacity
      {{ snapshot.confirmation.workload.capacity }} concurrent requests
    </p>

    <dl class="grid gap-3 sm:grid-cols-2">
      <div
        v-for="(value, name) in snapshot.arrival"
        :key="name"
        class="flex justify-between gap-4"
      >
        <dt class="text-muted-foreground capitalize">{{ name.replaceAll('_', ' ') }}</dt>
        <dd class="font-mono">{{ value }}</dd>
      </div>
    </dl>

    <p class="text-muted-foreground text-xs leading-relaxed">
      Offered
      {{
        snapshot.scheduling_us === '0'
          ? '0.00'
          : ((Number(snapshot.arrival.offered) * 1e6) / Number(snapshot.scheduling_us)).toFixed(2)
      }}
      iterations/s during scheduling; achieved
      {{ snapshot.metrics.request_rate.toFixed(2) }} requests/s including drain. Drops do not enter
      HTTP failure rate. Maximum lateness is in microseconds.
    </p>

    <p
      v-if="snapshot.arrival.capacity_drops !== '0' || snapshot.arrival.missed_deadlines !== '0'"
      class="text-destructive text-xs"
      role="status"
    >
      Generator saturation: requested load could not be offered. Capacity drops and missed deadlines
      are separate from target HTTP failures. Missed work is never replayed.
    </p>
  </div>
</template>
