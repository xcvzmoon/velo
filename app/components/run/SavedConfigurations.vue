<script setup lang="ts">
  import type { SavedConfiguration, TestDefinition } from '~/types/contracts';
  import { invoke } from '@tauri-apps/api/core';

  defineProps<{ disabled: boolean }>();
  const emit = defineEmits<{ load: [definition: TestDefinition] }>();

  const saved = shallowRef<SavedConfiguration[]>([]);
  const error = shallowRef<string | null>(null);
  const cursor = shallowRef<string | null>(null);
  const busy = shallowRef<boolean>(false);

  async function refresh(next = false): Promise<void> {
    try {
      saved.value = await invoke<SavedConfiguration[]>('list_configurations', {
        before: next ? cursor.value : null,
      });
      cursor.value = saved.value.at(-1)?.id ?? null;
    } catch {
      error.value = 'STORAGE_UNAVAILABLE: Saved configurations are unavailable.';
    }
  }

  async function load(id: string): Promise<void> {
    busy.value = true;
    error.value = null;
    try {
      const definition = await invoke<TestDefinition>('load_configuration_definition', { id });
      emit('load', definition);
    } catch {
      error.value =
        'LOAD_FAILED: Storage or remembered credentials are unavailable. Re-enter the request fields.';
    } finally {
      busy.value = false;
    }
  }

  async function remove(id: string): Promise<void> {
    busy.value = true;
    error.value = null;
    try {
      await invoke('delete_configuration', { id });
      await refresh();
    } catch {
      error.value = 'DELETE_FAILED: The configuration or its OS credential could not be removed.';
    } finally {
      busy.value = false;
    }
  }

  onMounted(() => refresh());
</script>

<template>
  <section
    class="space-y-4"
    aria-label="Saved configurations"
  >
    <p class="text-muted text-sm">
      Open an earlier saved configuration in the unified editor. Remembered fields remain OS-store
      references in definitions and exports. Select local upload files again. Save further changes
      under Saved tests. Configurations are listed in pages of 25.
    </p>

    <p
      v-if="error"
      class="text-error text-sm"
      role="alert"
    >
      {{ error }}
    </p>

    <ul class="space-y-3">
      <li
        v-for="entry in saved"
        :key="entry.id"
        class="flex flex-wrap items-center gap-3 text-sm"
      >
        <span class="flex-1">
          {{ entry.name }} · {{ entry.remembered ? 'OS credentials' : 'Sensitive fields omitted' }}
        </span>

        <UButton
          :disabled="disabled || busy"
          variant="outline"
          color="neutral"
          label="Load"
          @click="load(entry.id)"
        />

        <ShellConfirmAction
          :disabled="disabled || busy"
          label="Delete"
          title="Delete saved configuration?"
          description="This removes the saved configuration from this workspace."
          @confirm="remove(entry.id)"
        />
      </li>
    </ul>

    <UButton
      :disabled="busy || disabled || saved.length < 25"
      variant="outline"
      color="neutral"
      label="Older configurations"
      @click="refresh(true)"
    />

    <UButton
      :disabled="busy || disabled"
      variant="ghost"
      color="neutral"
      label="Latest configurations"
      @click="refresh()"
    />
  </section>
</template>
