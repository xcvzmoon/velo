<script setup lang="ts">
  import type { FormStore } from '@formisch/vue';
  import type { requestEditorSchema } from '~/schemas/run';
  import {
    Field as FormischField,
    FieldArray as FormischFieldArray,
    insert,
    remove,
  } from '@formisch/vue';

  defineProps<{ form: FormStore<typeof requestEditorSchema>; disabled: boolean }>();
</script>

<template>
  <FormischFieldArray
    v-slot="fieldArray"
    :of="form"
    :path="['multipartParts']"
  >
    <div class="space-y-4">
      <p class="text-muted text-xs leading-relaxed">
        Files stream from disk for each request and body-preserving redirect. Keep files unchanged
        during the run. Upload buffers count toward the request-data budget. The multipart boundary
        is managed by the client. File paths and contents are excluded from run reports.
      </p>

      <UButton
        :disabled="disabled"
        type="button"
        variant="outline"
        color="neutral"
        label="Add multipart field"
        @click="
          insert(form, {
            path: ['multipartParts'],
            initialInput: {
              kind: 'text',
              name: '',
              value: '',
              path: '',
              filename: '',
              content_type: 'application/octet-stream',
            },
          })
        "
      />

      <div
        v-for="(item, index) in fieldArray.items"
        :key="item"
        class="border-default space-y-3 rounded-md border p-4"
      >
        <div class="grid gap-3 sm:grid-cols-[1fr_1fr_auto]">
          <FormischField
            v-slot="field"
            :of="form"
            :path="['multipartParts', index, 'name']"
          >
            <div class="space-y-2">
              <div class="grid min-w-0 gap-2">
                <label
                  :for="`multipart-${item}-name`"
                  class="text-sm font-medium"
                >
                  Field name
                </label>

                <UInput
                  v-model="field.input"
                  v-bind="field.props"
                  :id="`multipart-${item}-name`"
                  autocomplete="off"
                  spellcheck="false"
                />
              </div>
            </div>
          </FormischField>

          <FormischField
            v-slot="field"
            :of="form"
            :path="['multipartParts', index, 'kind']"
          >
            <div class="space-y-2">
              <div class="grid min-w-0 gap-2">
                <label
                  :for="`multipart-${item}-kind`"
                  class="text-sm font-medium"
                >
                  Content
                </label>

                <USelect
                  v-model="field.input"
                  :disabled="disabled"
                  :name="field.props.name"
                  :id="`multipart-${item}-kind`"
                  :items="[
                    { label: 'Text', value: 'text' },
                    { label: 'File', value: 'file' },
                  ]"
                  class="w-full"
                />
              </div>
            </div>
          </FormischField>

          <UButton
            :disabled="disabled"
            :aria-label="`Remove multipart field ${index + 1}`"
            type="button"
            variant="ghost"
            class="sm:mt-6"
            color="neutral"
            label="Remove"
            @click="remove(form, { path: ['multipartParts'], at: index })"
          />
        </div>

        <FormischField
          v-slot="kind"
          :of="form"
          :path="['multipartParts', index, 'kind']"
        >
          <RunUploadField
            v-if="kind.input === 'file'"
            :form="form"
            :index="index"
            :item="item"
            :disabled="disabled"
          />

          <FormischField
            v-else
            v-slot="field"
            :of="form"
            :path="['multipartParts', index, 'value']"
          >
            <div class="space-y-2">
              <div class="grid min-w-0 gap-2">
                <label
                  :for="`multipart-${item}-value`"
                  class="text-sm font-medium"
                >
                  Value
                </label>

                <UTextarea
                  v-model="field.input"
                  v-bind="field.props"
                  :id="`multipart-${item}-value`"
                  autocomplete="off"
                  spellcheck="false"
                />
              </div>
            </div>
          </FormischField>
        </FormischField>
      </div>
    </div>
  </FormischFieldArray>
</template>
