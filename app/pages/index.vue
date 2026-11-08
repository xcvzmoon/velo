<script setup lang="ts">
  const { $runs } = useNuxtApp();
  const { snapshot } = $runs;
</script>

<template>
  <div>
    <ShellPageHeading
      title="Dashboard"
      description="A local studio for understanding how your APIs perform under load."
    />

    <RunMonitor v-if="snapshot" />

    <ShellEmptyState
      v-else
      title="Ready for your first run"
      description="Configure a GET or POST request, choose constant or ramping virtual users, and monitor live response times and throughput."
    >
      <UButton
        to="/tests"
        label="Configure a test"
        variant="outline"
      />
    </ShellEmptyState>
  </div>
</template>
