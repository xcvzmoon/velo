<script setup lang="ts">
  import type { ApiSessionView } from '~/types/contracts';
  import { invoke } from '@tauri-apps/api/core';

  const props = defineProps<{
    collectionId: string;
    environment: string | null;
    disabled: boolean;
    revision: string | null;
  }>();
  const session = shallowRef<ApiSessionView | null>(null);
  const error = shallowRef<string | null>(null);
  async function refresh(): Promise<void> {
    try {
      session.value = await invoke<ApiSessionView>('api_session', {
        collectionId: props.collectionId,
        environment: props.environment,
      });
    } catch (error_) {
      error.value = formatAuthoringError(error_);
    }
  }
  async function clear(): Promise<void> {
    try {
      await invoke('clear_api_session', {
        collectionId: props.collectionId,
        environment: props.environment,
      });
      await refresh();
    } catch (error_) {
      error.value = formatAuthoringError(error_);
    }
  }
  watch(() => [props.collectionId, props.environment, props.revision], refresh, {
    immediate: true,
  });
</script>

<template>
  <details class="space-y-3 rounded-lg border p-5">
    <summary class="cursor-pointer text-sm font-medium">
      Interactive session · {{ environment ?? 'shared variables' }}
    </summary>

    <p class="text-muted text-xs">
      Cookies and extracted variables stay in memory, isolated by collection and environment.
      Independent collection runs use separate sessions. Values are masked.
    </p>

    <p
      v-if="error"
      class="text-error text-sm"
      role="alert"
    >
      {{ error }}
    </p>

    <pre
      v-if="session"
      class="overflow-auto font-mono text-xs whitespace-pre-wrap"
      >{{ JSON.stringify(session, null, 2) }}</pre>

    <div class="flex gap-2">
      <UButton
        :disabled="disabled"
        variant="outline"
        color="neutral"
        label="Inspect session"
        @click="refresh"
      />

      <UButton
        :disabled="disabled"
        variant="outline"
        color="neutral"
        label="Clear cookies and extracted variables"
        @click="clear"
      />
    </div>
  </details>
</template>
