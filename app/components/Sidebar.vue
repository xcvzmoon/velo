<script setup lang="ts">
  defineProps<{ isSidebarCollapsed: boolean }>();

  const route = useRoute();
</script>

<template>
  <aside
    :class="isSidebarCollapsed ? 'w-16.25' : 'w-62.75'"
    class="bg-muted/40 border-muted/40 flex h-[inherit] shrink-0 flex-col gap-4 overflow-x-hidden overflow-y-auto border-r p-4 transition-[width]"
  >
    <ul
      id="workspace-navigation"
      class="flex flex-col gap-1"
    >
      <li
        v-for="{ label, to, icon } in workspaceNavigationItems"
        :key="to"
      >
        <UButton
          :to="to"
          :active="route.path === to"
          :icon="icon"
          :label="isSidebarCollapsed ? undefined : label"
          :aria-label="label"
          :size="isSidebarCollapsed ? 'icon' : undefined"
          color="neutral"
          active-color="primary"
          variant="ghost"
          active-variant="soft"
          class="w-full justify-start px-2"
        />
      </li>
    </ul>
  </aside>
</template>
