<script setup lang="ts">
  const props = defineProps<{ snapshot: RunSnapshot }>();
  const rates = computed<GeneratorArrivalRates>(() => generatorArrivalRates(props.snapshot));
</script>

<template>
  <div
    v-if="snapshot.confirmation.workload.kind === 'arrival'"
    class="@container space-y-3 rounded-lg border p-4 text-sm"
  >
    <p>
      Target {{ snapshot.confirmation.workload.rate }} iterations/s · capacity
      {{ snapshot.confirmation.workload.capacity }} concurrent iterations
    </p>

    <dl class="grid gap-3 @min-[28rem]:grid-cols-2">
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
      Requested {{ snapshot.confirmation.workload.rate }} iterations/s. Offered
      {{ rates.offered?.toFixed(2) ?? 'Unavailable' }}
      iterations/s; started {{ rates.started?.toFixed(2) ?? 'Unavailable' }}
      iterations/s. Offered and started iteration rates both use the scheduling-only window.
      Scenario iterations can contain multiple HTTP requests. Completed HTTP rates include drain and
      use a different window and population. Drops do not enter HTTP or assertion failure rates.
      Maximum scheduler lateness is in microseconds.
    </p>

    <p
      v-if="snapshot.arrival.capacity_drops !== '0' || snapshot.arrival.missed_deadlines !== '0'"
      class="text-destructive text-xs"
      role="status"
    >
      Admission pressure observed: capacity drops or missed arrival slots. Capacity drops and missed
      deadlines are separate from target HTTP failures. Missed work is never replayed.
    </p>
  </div>
</template>
