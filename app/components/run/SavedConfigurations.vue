<script setup lang="ts">
  import type { InferOutput } from 'valibot';
  import type { RunConfig, SavedConfiguration } from '~/types/contracts';
  import { Field as FormischField, Form as FormischForm, useForm } from '@formisch/vue';
  import { invoke } from '@tauri-apps/api/core';
  import * as v from 'valibot';

  const schema = v.object({
    name: v.pipe(v.string(), v.trim(), v.minLength(1), v.maxLength(80)),
    remember: v.picklist(['omit', 'remember']),
  });

  const props = defineProps<{ config: RunConfig | null; disabled: boolean }>();
  const emit = defineEmits<{ load: [config: RunConfig] }>();

  const form = useForm({
    schema,
    initialInput: {
      name: '',
      remember: 'omit',
    },
  });
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

  async function save(values: InferOutput<typeof schema>): Promise<void> {
    if (!props.config) {
      return;
    }
    busy.value = true;
    error.value = null;
    try {
      await invoke('save_configuration', {
        name: values.name,
        config: props.config,
        remember: values.remember === 'remember',
      });
      await refresh();
    } catch {
      error.value =
        'SAVE_FAILED: The configuration could not be saved. Check local storage and OS credential access.';
    } finally {
      busy.value = false;
    }
  }

  async function load(id: string): Promise<void> {
    busy.value = true;
    error.value = null;
    try {
      const entry = await invoke<SavedConfiguration>('load_configuration', { id });
      emit('load', entry.config);
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
    class="space-y-4 rounded-lg border p-6"
    aria-label="Saved configurations"
  >
    <h2 class="font-medium">Saved configurations</h2>

    <p class="text-muted-foreground text-sm">
      Review a request to save it. By default, saves omit the URL path, query, headers,
      authentication, and body. Remembering stores the full request in your OS credential store. Use
      a name without secrets. Configurations are listed in pages of 25.
    </p>

    <p
      v-if="error"
      class="text-destructive text-sm"
      role="alert"
    >
      {{ error }}
    </p>

    <FormischForm
      :of="form"
      class="flex flex-wrap items-end gap-3"
      @submit="save"
    >
      <FormischField
        v-slot="field"
        :of="form"
        :path="['name']"
      >
        <div class="space-y-2">
          <Label for="configuration-name">Name</Label>

          <Input
            v-model="field.input"
            v-bind="field.props"
            :disabled="disabled || busy"
            id="configuration-name"
          />

          <p
            v-if="field.errors"
            class="text-destructive text-sm"
          >
            {{ field.errors.join(' ') }}
          </p>
        </div>
      </FormischField>

      <FormischField
        v-slot="field"
        :of="form"
        :path="['remember']"
      >
        <div class="space-y-2">
          <Label for="configuration-secrets">Request data</Label>

          <Select
            v-model="field.input"
            :disabled="disabled || busy"
          >
            <SelectTrigger id="configuration-secrets"><SelectValue /></SelectTrigger>

            <SelectContent>
              <SelectItem value="omit">Omit sensitive fields</SelectItem>
              <SelectItem value="remember">Remember full request in OS store</SelectItem>
            </SelectContent>
          </Select>
        </div>
      </FormischField>

      <Button
        :disabled="disabled || busy || !config"
        type="submit"
      >
        Save reviewed request
      </Button>
    </FormischForm>

    <ul class="space-y-3">
      <li
        v-for="entry in saved"
        :key="entry.id"
        class="flex flex-wrap items-center gap-3 text-sm"
      >
        <span class="flex-1">
          {{ entry.name }} · {{ entry.remembered ? 'OS credentials' : 'Sensitive fields omitted' }}
        </span>

        <Button
          :disabled="disabled || busy"
          variant="outline"
          @click="load(entry.id)"
        >
          Load
        </Button>

        <Button
          :disabled="disabled || busy"
          variant="ghost"
          @click="remove(entry.id)"
        >
          Delete
        </Button>
      </li>
    </ul>

    <Button
      :disabled="busy || disabled || saved.length < 25"
      variant="outline"
      @click="refresh(true)"
    >
      Older configurations
    </Button>

    <Button
      :disabled="busy || disabled"
      variant="ghost"
      @click="refresh()"
    >
      Latest configurations
    </Button>
  </section>
</template>
