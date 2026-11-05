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
      description="Configure a GET or POST request, choose a constant virtual-user workload, and monitor live response times and throughput."
    >
      <Button
        variant="outline"
        as-child
      >
        <NuxtLink to="/tests">Configure a test</NuxtLink>
      </Button>
    </ShellEmptyState>
  </div>
</template>
