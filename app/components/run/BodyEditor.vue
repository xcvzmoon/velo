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
  <FormischField
    v-slot="mode"
    :of="form"
    :path="['bodyKind']"
  >
    <div class="border-default space-y-4 border-t pt-6">
      <div class="max-w-xs space-y-2">
        <div class="grid min-w-0 gap-2">
          <label
            class="text-sm font-medium"
            for="request-body-kind"
          >
            Body format
          </label>

          <USelect
            v-model="mode.input"
            :disabled="disabled"
            :name="mode.props.name"
            :aria-invalid="Boolean(mode.errors)"
            :items="[
              { label: 'No body', value: 'none' },
              { label: 'Plain text', value: 'text' },
              { label: 'JSON', value: 'json' },
              { label: 'URL-encoded form', value: 'form' },
              { label: 'Multipart / files', value: 'multipart' },
            ]"
            id="request-body-kind"
            aria-describedby="request-body-kind-error"
            class="w-full"
          />
        </div>

        <p
          v-if="mode.errors"
          id="request-body-kind-error"
          class="text-error text-xs"
          role="alert"
        >
          {{ mode.errors[0] }}
        </p>
      </div>

      <FormischField
        v-if="mode.input === 'text' || mode.input === 'json'"
        v-slot="field"
        :of="form"
        :path="['bodyValue']"
      >
        <div class="space-y-2">
          <div class="grid min-w-0 gap-2">
            <label
              class="text-sm font-medium"
              for="request-body"
            >
              Request body
            </label>

            <UTextarea
              v-model="field.input"
              v-bind="field.props"
              :aria-invalid="Boolean(field.errors)"
              :ui="{ base: 'font-mono' }"
              id="request-body"
              aria-describedby="request-body-error"
              autocomplete="off"
              spellcheck="false"
            />
          </div>

          <p
            v-if="field.errors"
            id="request-body-error"
            class="text-error text-xs"
            role="alert"
          >
            {{ field.errors[0] }}
          </p>
        </div>
      </FormischField>

      <FormischFieldArray
        v-if="mode.input === 'form'"
        v-slot="fieldArray"
        :of="form"
        :path="['formPairs']"
      >
        <div class="space-y-3">
          <UButton
            :disabled="disabled"
            type="button"
            variant="outline"
            color="neutral"
            label="Add form field"
            @click="insert(form, { path: ['formPairs'], initialInput: { name: '', value: '' } })"
          />

          <div
            v-for="(item, index) in fieldArray.items"
            :key="item"
            class="grid gap-3 sm:grid-cols-[1fr_1fr_auto]"
          >
            <FormischField
              v-for="property in ['name', 'value'] as const"
              v-slot="field"
              :key="property"
              :of="form"
              :path="['formPairs', index, property]"
            >
              <div class="space-y-2">
                <div class="grid min-w-0 gap-2">
                  <label
                    :for="`form-${item}-${property}`"
                    class="text-sm font-medium"
                  >
                    {{ property === 'name' ? 'Name' : 'Value' }}
                  </label>

                  <UInput
                    v-model="field.input"
                    v-bind="field.props"
                    :id="`form-${item}-${property}`"
                    autocomplete="off"
                    spellcheck="false"
                  />
                </div>
              </div>
            </FormischField>

            <UButton
              :disabled="disabled"
              :aria-label="`Remove form field ${index + 1}`"
              type="button"
              variant="ghost"
              class="sm:mt-6"
              color="neutral"
              label="Remove"
              @click="remove(form, { path: ['formPairs'], at: index })"
            />
          </div>
        </div>
      </FormischFieldArray>

      <RunMultipartEditor
        v-if="mode.input === 'multipart'"
        :form="form"
        :disabled="disabled"
      />

      <p class="text-muted text-xs leading-relaxed">
        GET and HEAD do not accept bodies. Other methods support every body format. Duplicate form
        and multipart names retain their order. Inactive body and authentication values are not
        sent.
      </p>
    </div>
  </FormischField>
</template>
