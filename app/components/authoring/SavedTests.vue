<script setup lang="ts">
  import type { InferOutput } from 'valibot';
  import type { TestDefinition } from '~/types/contracts';
  import { Field as FormischField, Form as FormischForm, useForm } from '@formisch/vue';
  import { invoke } from '@tauri-apps/api/core';
  import * as v from 'valibot';

  const nameSchema = v.object({
    name: v.pipe(v.string(), v.trim(), v.minLength(1), v.maxLength(80)),
  });
  const props = defineProps<{ source: string; disabled: boolean; canSave: boolean }>();
  const emit = defineEmits<{ load: [definition: TestDefinition] }>();
  const form = useForm({
    schema: nameSchema,
    initialInput: { name: '' },
  });
  const names = shallowRef<string[]>([]);
  const chosen = shallowRef<string>('');
  const error = shallowRef<string | null>(null);
  const busy = shallowRef<boolean>(false);

  async function perform(action: () => Promise<void>): Promise<void> {
    busy.value = true;
    error.value = null;
    try {
      await action();
    } catch (error_) {
      error.value = formatAuthoringError(error_);
    } finally {
      busy.value = false;
    }
  }

  async function refresh(): Promise<void> {
    names.value = await invoke<string[]>('list_definitions');
  }

  async function save(values: InferOutput<typeof nameSchema>): Promise<void> {
    await perform(async () => {
      await invoke('save_definition', {
        name: values.name,
        source: props.source,
      });
      await refresh();
    });
  }

  async function load(): Promise<void> {
    await perform(async () =>
      emit('load', await invoke<TestDefinition>('load_definition', { name: chosen.value })),
    );
  }

  async function remove(): Promise<void> {
    await perform(async () => {
      await invoke('delete_definition', { name: chosen.value });
      chosen.value = '';
      await refresh();
    });
  }

  onMounted(() => perform(refresh));
</script>

<template>
  <section
    class="space-y-4 rounded-lg border p-6"
    aria-label="Saved test definitions"
  >
    <h2 class="font-medium">Saved tests</h2>

    <p
      v-if="error"
      class="text-destructive text-sm"
      role="alert"
    >
      {{ error }}
    </p>

    <p class="text-muted-foreground text-sm">
      Definitions store literal request fields and non-secret variables locally. Use secret
      references for private values. Saving under an existing name replaces that definition.
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
          <Label for="test-name">Test name</Label>

          <Input
            v-model="field.input"
            v-bind="field.props"
            :disabled="disabled || busy"
            id="test-name"
          />

          <p
            v-if="field.errors"
            class="text-destructive text-sm"
          >
            {{ field.errors.join(' ') }}
          </p>
        </div>
      </FormischField>

      <Button
        :disabled="disabled || busy || !canSave"
        type="submit"
      >
        Save test
      </Button>
    </FormischForm>

    <template v-if="names.length">
      <Label for="saved-test">Saved test</Label>

      <Select
        v-model="chosen"
        :disabled="disabled || busy"
      >
        <SelectTrigger id="saved-test"><SelectValue placeholder="Choose a test" /></SelectTrigger>

        <SelectContent>
          <SelectItem
            v-for="name in names"
            :key="name"
            :value="name"
          >
            {{ name }}
          </SelectItem>
        </SelectContent>
      </Select>

      <div class="flex gap-3">
        <Button
          :disabled="disabled || busy || !chosen"
          variant="outline"
          @click="load"
        >
          Load test
        </Button>
        <Button
          :disabled="disabled || busy || !chosen"
          variant="ghost"
          @click="remove"
        >
          Delete test
        </Button>
      </div>
    </template>
  </section>
</template>
