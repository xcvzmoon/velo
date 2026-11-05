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
    <div class="space-y-4 border-t pt-6">
      <div class="max-w-xs space-y-2">
        <Label for="request-body-kind">Body format</Label>

        <Select
          v-model="mode.input"
          :disabled="disabled"
          :name="mode.props.name"
        >
          <SelectTrigger
            :aria-invalid="Boolean(mode.errors)"
            id="request-body-kind"
            aria-describedby="request-body-kind-error"
            class="w-full"
          >
            <SelectValue />
          </SelectTrigger>

          <SelectContent>
            <SelectItem value="none">No body</SelectItem>
            <SelectItem value="text">Plain text</SelectItem>
            <SelectItem value="json">JSON</SelectItem>
            <SelectItem value="form">URL-encoded form</SelectItem>
            <SelectItem value="multipart">Multipart / files</SelectItem>
          </SelectContent>
        </Select>

        <p
          v-if="mode.errors"
          id="request-body-kind-error"
          class="text-destructive text-xs"
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
          <Label for="request-body">Request body</Label>

          <Textarea
            v-model="field.input"
            v-bind="field.props"
            :aria-invalid="Boolean(field.errors)"
            id="request-body"
            aria-describedby="request-body-error"
            class="min-h-28 font-mono text-xs"
            autocomplete="off"
            spellcheck="false"
          />

          <p
            v-if="field.errors"
            id="request-body-error"
            class="text-destructive text-xs"
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
          <Button
            :disabled="disabled"
            type="button"
            size="sm"
            variant="outline"
            @click="insert(form, { path: ['formPairs'], initialInput: { name: '', value: '' } })"
          >
            Add form field
          </Button>

          <div
            v-for="(item, index) in fieldArray.items"
            :key="item"
            class="grid gap-3 sm:grid-cols-[1fr_1fr_auto]"
          >
            <FormischField
              v-for="property in ['name', 'value'] as const"
              :key="property"
              v-slot="field"
              :of="form"
              :path="['formPairs', index, property]"
            >
              <div class="space-y-2">
                <Label :for="`form-${item}-${property}`">
                  {{ property === 'name' ? 'Name' : 'Value' }}
                </Label>

                <Input
                  v-model="field.input"
                  v-bind="field.props"
                  :id="`form-${item}-${property}`"
                  autocomplete="off"
                  spellcheck="false"
                />
              </div>
            </FormischField>

            <Button
              :disabled="disabled"
              :aria-label="`Remove form field ${index + 1}`"
              type="button"
              variant="ghost"
              class="sm:mt-6"
              @click="remove(form, { path: ['formPairs'], at: index })"
            >
              Remove
            </Button>
          </div>
        </div>
      </FormischFieldArray>

      <RunMultipartEditor
        v-if="mode.input === 'multipart'"
        :form="form"
        :disabled="disabled"
      />

      <p class="text-muted-foreground text-xs leading-relaxed">
        GET and HEAD do not accept bodies. Other methods support every body format. Duplicate form
        and multipart names retain their order. Inactive body and authentication values are not
        sent.
      </p>
    </div>
  </FormischField>
</template>
