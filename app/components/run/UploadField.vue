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
        <Label :for="`multipart-${item}-path`">Local file</Label>

        <div class="flex gap-2">
          <Input
            v-model="field.input"
            v-bind="field.props"
            :id="`multipart-${item}-path`"
            :aria-describedby="`multipart-${item}-file-error`"
            readonly
            placeholder="Choose a file"
          />

          <Button
            :disabled="disabled || selecting"
            type="button"
            variant="outline"
            @click="selectFile"
          >
            {{ selecting ? 'Selecting…' : 'Choose file' }}
          </Button>
        </div>
      </div>
    </FormischField>

    <p
      v-if="error"
      :id="`multipart-${item}-file-error`"
      class="text-destructive text-xs"
      role="alert"
    >
      {{ error }}
    </p>

    <div class="grid gap-3 sm:grid-cols-2">
      <FormischField
        v-for="property in ['filename', 'content_type'] as const"
        :key="property"
        v-slot="field"
        :of="form"
        :path="['multipartParts', index, property]"
      >
        <div class="space-y-2">
          <Label :for="`multipart-${item}-${property}`">
            {{ property === 'filename' ? 'Upload filename' : 'MIME type' }}
          </Label>

          <Input
            v-model="field.input"
            v-bind="field.props"
            :id="`multipart-${item}-${property}`"
            autocomplete="off"
            spellcheck="false"
          />
        </div>
      </FormischField>
    </div>
  </div>
</template>
