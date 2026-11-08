<script setup lang="ts">
  import type { FormStore } from '@formisch/vue';
  import type { requestEditorSchema } from '~/schemas/run';
  import { Field as FormischField, setInput } from '@formisch/vue';
  import { invoke } from '@tauri-apps/api/core';

  const props = defineProps<{
    form: FormStore<typeof requestEditorSchema>;
    index: number;
    item: string;
    disabled: boolean;
  }>();

  const selecting = shallowRef<boolean>(false);
  const error = shallowRef<string | null>(null);
  let disposed = false;

  async function selectFile(): Promise<void> {
    if (selecting.value) {
      return;
    }

    selecting.value = true;
    error.value = null;

    try {
      const path = await invoke<string | null>('select_upload_file');
      if (disposed || path === null) {
        return;
      }

      setInput(props.form, {
        path: ['multipartParts', props.index, 'path'],
        input: path,
      });
      setInput(props.form, {
        path: ['multipartParts', props.index, 'filename'],
        input: path.replaceAll('\\', '/').split('/').at(-1) ?? '',
      });
    } catch {
      error.value = 'The file selector could not complete. Try again.';
    } finally {
      selecting.value = false;
    }
  }

  onUnmounted(() => {
    disposed = true;
  });
</script>

<template>
  <div class="space-y-3">
    <FormischField
      v-slot="field"
      :of="form"
      :path="['multipartParts', index, 'path']"
    >
      <div class="space-y-2">
        <label
          :for="`multipart-${item}-path`"
          class="text-sm font-medium"
        >
          Local file
        </label>

        <div class="flex gap-2">
          <UInput
            v-model="field.input"
            v-bind="field.props"
            :id="`multipart-${item}-path`"
            :aria-describedby="`multipart-${item}-file-error`"
            placeholder="Choose a file"
            readonly
          />

          <UButton
            :disabled="disabled || selecting"
            :label="selecting ? 'Selecting…' : 'Choose file'"
            type="button"
            variant="outline"
            color="neutral"
            @click="selectFile"
          />
        </div>
      </div>
    </FormischField>

    <p
      v-if="error"
      :id="`multipart-${item}-file-error`"
      class="text-error text-xs"
      role="alert"
    >
      {{ error }}
    </p>

    <div class="grid gap-3 sm:grid-cols-2">
      <FormischField
        v-for="property in ['filename', 'content_type'] as const"
        v-slot="field"
        :key="property"
        :of="form"
        :path="['multipartParts', index, property]"
      >
        <div class="space-y-2">
          <div class="grid min-w-0 gap-2">
            <label
              :for="`multipart-${item}-${property}`"
              class="text-sm font-medium"
            >
              {{ property === 'filename' ? 'Upload filename' : 'MIME type' }}
            </label>

            <UInput
              v-model="field.input"
              v-bind="field.props"
              :id="`multipart-${item}-${property}`"
              autocomplete="off"
              spellcheck="false"
            />
          </div>
        </div>
      </FormischField>
    </div>
  </div>
</template>
