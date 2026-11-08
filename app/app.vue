<script setup lang="ts">
  const navigationItems = [
    {
      label: 'Overview',
      to: '/',
      icon: 'i-hugeicons-dashboard-square-01',
    },
    {
      label: 'API workspace',
      to: '/api',
      icon: 'i-hugeicons-code',
    },
    {
      label: 'Test studio',
      to: '/tests',
      icon: 'i-hugeicons-test-tube-01',
    },
    {
      label: 'Results',
      to: '/results',
      icon: 'i-hugeicons-chart-line-data-01',
    },
    {
      label: 'Settings',
      to: '/settings',
      icon: 'i-hugeicons-settings-02',
    },
  ];

  const pageViewport = useTemplateRef<HTMLDivElement>('pageViewport');
  const scrollbarWidth = ref<number>(0);

  useResizeObserver(pageViewport, () => {
    const viewport = pageViewport.value;
    if (viewport) {
      scrollbarWidth.value = viewport.offsetWidth - viewport.clientWidth;
    }
  });

  const route = useRoute();
  const isSidebarCollapsed = useLocalStorage<boolean>('sidebar:is-collapsed', () => false);
</script>

<template>
  <UApp>
    <Topbar v-model:isSidebarCollapsed="isSidebarCollapsed" />

    <div class="bg-background flex h-[calc(100svh-2.5rem)]">
      <aside
        :class="isSidebarCollapsed ? 'w-16.25' : 'w-62.75'"
        class="bg-muted/40 border-muted/40 flex h-[inherit] shrink-0 flex-col gap-4 overflow-x-hidden overflow-y-auto border-r p-4 transition-[width]"
      >
        <ul
          id="workspace-navigation"
          class="flex flex-col gap-1"
        >
          <li
            v-for="{ label, to, icon } in navigationItems"
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

      <div class="h-full min-h-0 min-w-0 flex-1 overflow-hidden p-5.75 pr-0">
        <div
          ref="pageViewport"
          class="h-full min-w-0 scrollbar-gutter-stable overflow-auto"
        >
          <div
            :style="{ paddingRight: `max(1px, calc(24px - ${scrollbarWidth}px))` }"
            class="min-h-full min-w-0 p-px"
          >
            <NuxtPage />
          </div>
        </div>
      </div>
    </div>
  </UApp>
</template>
