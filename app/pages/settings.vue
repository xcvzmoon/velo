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
  <div class="mx-auto max-w-7xl min-w-0 space-y-6">
    <ShellPageHeading
      title="Settings"
      description="Manage this workspace, verify the native connection, and configure optional AI assistance."
    />

    <UTabs
      :items="sections"
      :unmount-on-hide="false"
      :ui="{
        list: 'max-w-full overflow-x-auto rounded-xl bg-elevated/60 ring ring-inset ring-default',
        trigger: 'shrink-0',
        content: 'min-w-0 space-y-6',
      }"
      variant="pill"
      color="neutral"
      class="gap-6"
    >
      <template #workspace>
        <div class="grid min-w-0 items-start gap-6 xl:grid-cols-2">
          <ShellPanel aria-label="Appearance settings">
            <template #header>
              <h2 class="text-highlighted text-sm font-semibold">Appearance</h2>
              <p class="text-muted mt-1 text-xs">Choose how this workspace looks on your device.</p>
            </template>

            <div class="max-w-sm space-y-2">
              <div class="grid min-w-0 gap-2">
                <label
                  for="workspace-theme"
                  class="block text-sm font-medium"
                >
                  Color theme
                </label>

                <UColorModeSelect
                  id="workspace-theme"
                  class="w-full"
                />
              </div>
            </div>
          </ShellPanel>

          <ShellDiagnostic />
        </div>
      </template>

      <template #ai>
        <AiSettings />
      </template>
    </UTabs>
  </div>
</template>
