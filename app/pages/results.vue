<script setup lang="ts">
  const sections = [
    {
      label: 'Latest run',
      slot: 'latest',
    },
    {
      label: 'History & comparisons',
      slot: 'history',
    },
  ];

  definePageMeta({ keepalive: true });

  const { $runs } = useNuxtApp();
  const { snapshot } = $runs;
</script>

<template>
  <div>
    <ShellPageHeading
      title="Results"
      description="Inspect measurements, compare against a baseline, and export run evidence."
    />

    <UTabs
      :items="sections"
      :unmount-on-hide="false"
      variant="link"
      class="gap-6"
    >
      <template #latest>
        <template v-if="snapshot">
          <RunMonitor />
          <RunSummary />
        </template>

        <ShellEmptyState
          v-else
          title="No results yet"
          description="Configure and run a request in Tests. Stored runs appear in history below."
        >
          <UButton
            variant="outline"
            to="/tests"
            color="neutral"
            label="Configure a test"
          />
        </ShellEmptyState>
      </template>

      <template #history>
        <RunHistory />
      </template>
    </UTabs>
  </div>
</template>
