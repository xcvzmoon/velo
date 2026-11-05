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
      <p class="text-muted-foreground text-xs leading-relaxed">
        Files stream from disk for each request and body-preserving redirect. Keep files unchanged
        during the run. Upload buffers count toward the request-data budget. The multipart boundary
        is managed by the client. File paths and contents are excluded from run reports.
      </p>

      <Button
        :disabled="disabled"
        type="button"
        size="sm"
        variant="outline"
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
      >
        Add multipart field
      </Button>

      <div
        v-for="(item, index) in fieldArray.items"
        :key="item"
        class="space-y-3 rounded-md border p-4"
      >
        <div class="grid gap-3 sm:grid-cols-[1fr_1fr_auto]">
          <FormischField
            v-slot="field"
            :of="form"
            :path="['multipartParts', index, 'name']"
          >
            <div class="space-y-2">
              <Label :for="`multipart-${item}-name`">Field name</Label>

              <Input
                v-model="field.input"
                v-bind="field.props"
                :id="`multipart-${item}-name`"
                autocomplete="off"
                spellcheck="false"
              />
            </div>
          </FormischField>

          <FormischField
            v-slot="field"
            :of="form"
            :path="['multipartParts', index, 'kind']"
          >
            <div class="space-y-2">
              <Label :for="`multipart-${item}-kind`">Content</Label>

              <Select
                v-model="field.input"
                :disabled="disabled"
                :name="field.props.name"
              >
                <SelectTrigger
                  :id="`multipart-${item}-kind`"
                  class="w-full"
                >
                  <SelectValue />
                </SelectTrigger>

                <SelectContent>
                  <SelectItem value="text">Text</SelectItem>
                  <SelectItem value="file">File</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </FormischField>

          <Button
            :disabled="disabled"
            :aria-label="`Remove multipart field ${index + 1}`"
            type="button"
            variant="ghost"
            class="sm:mt-6"
            @click="remove(form, { path: ['multipartParts'], at: index })"
          >
            Remove
          </Button>
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
              <Label :for="`multipart-${item}-value`">Value</Label>

              <Textarea
                v-model="field.input"
                v-bind="field.props"
                :id="`multipart-${item}-value`"
                autocomplete="off"
                spellcheck="false"
              />
            </div>
          </FormischField>
        </FormischField>
      </div>
    </div>
  </FormischFieldArray>
</template>
