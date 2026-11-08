<script setup lang="ts">
  const sections = [
    {
      label: 'Workspace',
      slot: 'workspace',
    },
    {
      label: 'AI providers',
      slot: 'ai',
    },
  ];
</script>

<template>
  <div class="space-y-6">
    <ShellPageHeading
      title="Settings"
      description="Manage this workspace, verify the native connection, and configure optional AI assistance."
    />

    <UTabs
      :items="sections"
      :unmount-on-hide="false"
      variant="link"
      class="gap-6"
    >
      <template #workspace>
        <UCard
          class="mb-6"
          variant="subtle"
        >
          <h2 class="text-highlighted text-sm font-semibold">Appearance</h2>
          <p class="text-muted mt-2 mb-4 text-sm">Choose the theme for this device.</p>
          <UColorModeSelect class="w-48" />
        </UCard>

        <ShellDiagnostic />
      </template>

      <template #ai>
        <AiSettings />
      </template>
    </UTabs>
  </div>
</template>
