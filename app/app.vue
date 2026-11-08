<script setup lang="ts">
  const pageViewport = useTemplateRef<HTMLDivElement>('pageViewport');
  const scrollbarWidth = ref<number>(0);

  useResizeObserver(pageViewport, () => {
    const viewport = pageViewport.value;
    if (viewport) {
      scrollbarWidth.value = viewport.offsetWidth - viewport.clientWidth;
    }
  });

  const isSidebarCollapsed = useLocalStorage<boolean>('sidebar:is-collapsed', () => false);
</script>

<template>
  <UApp>
    <Topbar v-model:isSidebarCollapsed="isSidebarCollapsed" />

    <div class="bg-background flex h-[calc(100svh-2.5rem)]">
      <Sidebar :is-sidebar-collapsed="isSidebarCollapsed" />

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
