<script setup lang="ts">
  const isSidebarCollapsed = defineModel<boolean>('isSidebarCollapsed', { required: true });

  const { $runs } = useNuxtApp();
  const { active } = $runs;
</script>

<template>
  <header
    class="bg-muted/40 border-muted/40 flex h-10 items-center justify-between border-b ps-21 pe-3"
    data-tauri-drag-region
  >
    <div class="flex items-center gap-2">
      <UButton
        size="sm"
        color="neutral"
        variant="ghost"
        icon="i-hugeicons-arrow-left-01"
        class="p-1.5"
      />

      <UButton
        size="sm"
        color="neutral"
        variant="ghost"
        icon="i-hugeicons-arrow-right-01"
        class="p-1.5"
      />

      <UButton
        :icon="isSidebarCollapsed ? 'i-hugeicons-panel-right' : 'i-hugeicons-panel-left'"
        size="sm"
        color="neutral"
        variant="ghost"
        class="p-1.5"
        @click="isSidebarCollapsed = !isSidebarCollapsed"
      />

      <UBadge
        size="sm"
        color="neutral"
        variant="subtle"
        label="Workspace"
      />
    </div>

    <div class="flex items-center gap-1">
      <UBadge
        v-if="active"
        color="primary"
        variant="subtle"
        size="sm"
      >
        Run in progress
      </UBadge>

      <ColorModeButton />
    </div>
  </header>
</template>
