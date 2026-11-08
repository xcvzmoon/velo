<script setup lang="ts">
  import * as v from 'valibot';

  const navigationStateSchema = v.object({
    back: v.nullish(v.string()),
    forward: v.nullish(v.string()),
  });
  const isSidebarCollapsed = defineModel<boolean>('isSidebarCollapsed', { required: true });

  const { $runs } = useNuxtApp();
  const route = useRoute();
  const router = useRouter();
  const { active } = $runs;
  const canGoBack = ref<boolean>(false);
  const canGoForward = ref<boolean>(false);
  const workspace = computed<string>(
    () => workspaceNavigationItems.find(({ to }) => to === route.path)?.label ?? 'Workspace',
  );

  function syncNavigation(): void {
    const navigation = v.safeParse(navigationStateSchema, router.options.history.state);
    canGoBack.value = navigation.success && Boolean(navigation.output.back);
    canGoForward.value = navigation.success && Boolean(navigation.output.forward);
  }

  const removeNavigationHook = router.afterEach(syncNavigation);
  onMounted(syncNavigation);
  onScopeDispose(removeNavigationHook);
</script>

<template>
  <header
    class="bg-muted/40 border-muted/40 flex h-10 items-center justify-between border-b ps-21 pe-3"
    data-tauri-drag-region
  >
    <div class="flex items-center gap-2">
      <UButton
        :disabled="!canGoBack"
        color="neutral"
        variant="ghost"
        icon="i-hugeicons-arrow-left-01"
        aria-label="Go back"
        class="p-1.5"
        @click="router.back()"
      />

      <UButton
        :disabled="!canGoForward"
        color="neutral"
        variant="ghost"
        icon="i-hugeicons-arrow-right-01"
        aria-label="Go forward"
        class="p-1.5"
        @click="router.forward()"
      />

      <UButton
        :aria-label="isSidebarCollapsed ? 'Expand sidebar' : 'Collapse sidebar'"
        :icon="isSidebarCollapsed ? 'i-hugeicons-panel-right' : 'i-hugeicons-panel-left'"
        color="neutral"
        variant="ghost"
        class="p-1.5"
        @click="isSidebarCollapsed = !isSidebarCollapsed"
      />

      <UBadge
        :label="workspace"
        size="sm"
        color="neutral"
        variant="subtle"
        role="status"
        aria-label="Active workspace"
        aria-live="polite"
        aria-atomic="true"
      />
    </div>

    <div class="flex items-center gap-1">
      <UBadge
        v-if="active"
        color="primary"
        variant="subtle"
        size="sm"
        label="Run in progress"
      />

      <ColorModeButton />
    </div>
  </header>
</template>
