<script setup lang="ts">
  const { $runs } = useNuxtApp();
  const { snapshot } = $runs;
</script>

<template>
  <div>
    <ShellPageHeading
      title="Results"
      description="Inspect the current run and its final measurements."
    />

    <template v-if="snapshot">
      <RunMonitor />
      <RunSummary />
    </template>

    <ShellEmptyState
      v-else
      title="No results yet"
      description="Configure and run a request in Tests. Stored runs appear in history below."
    >
      <Button
        variant="outline"
        as-child
      >
        <NuxtLink to="/tests">Configure a test</NuxtLink>
      </Button>
    </ShellEmptyState>

    <RunHistory />
  </div>
</template>
