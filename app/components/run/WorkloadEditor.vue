<script setup lang="ts">
  import type { FormStore } from '@formisch/vue';
  import type { requestEditorSchema } from '~/schemas/run';
  import {
    Field as FormischField,
    FieldArray as FormischFieldArray,
    insert,
    remove,
  } from '@formisch/vue';

  const constantFields = [
    {
      name: 'vus',
      label: 'Virtual users',
    },
    {
      name: 'duration_ms',
      label: 'Scheduling duration (ms)',
    },
  ] as const;

  const stageFields = [
    {
      name: 'duration_ms',
      label: 'Duration (ms)',
      min: 1,
    },
    {
      name: 'target_vus',
      label: 'Target VUs',
      min: 0,
    },
  ] as const;

  defineProps<{ form: FormStore<typeof requestEditorSchema>; disabled: boolean }>();
</script>

<template>
  <FormischField
    v-slot="mode"
    :of="form"
    :path="['workloadKind']"
  >
    <div class="space-y-5">
      <div class="max-w-xs space-y-2">
        <Label for="workload-kind">Workload</Label>

        <Select
          v-model="mode.input"
          :disabled="disabled"
          :name="mode.props.name"
        >
          <SelectTrigger
            id="workload-kind"
            class="w-full"
          >
            <SelectValue />
          </SelectTrigger>

          <SelectContent>
            <SelectItem value="constant">Constant VUs</SelectItem>
            <SelectItem value="ramping">Ramping VUs</SelectItem>
          </SelectContent>
        </Select>
      </div>

      <div
        v-if="mode.input === 'constant'"
        class="grid gap-4 sm:grid-cols-2"
      >
        <FormischField
          v-for="option in constantFields"
          :key="option.name"
          v-slot="field"
          :of="form"
          :path="[option.name]"
        >
          <div class="space-y-2">
            <Label :for="`request-${option.name}`">{{ option.label }}</Label>

            <Input
              v-model="field.input"
              v-bind="field.props"
              :id="`request-${option.name}`"
              :aria-invalid="Boolean(field.errors)"
              :aria-describedby="`request-${option.name}-error`"
              type="number"
              min="1"
              step="1"
            />

            <p
              v-if="field.errors"
              :id="`request-${option.name}-error`"
              class="text-destructive text-xs"
              role="alert"
            >
              {{ field.errors[0] }}
            </p>
          </div>
        </FormischField>
      </div>

      <div
        v-else
        class="space-y-5"
      >
        <FormischField
          v-slot="field"
          :of="form"
          :path="['start_vus']"
        >
          <div class="max-w-xs space-y-2">
            <Label for="start-vus">Starting VUs</Label>

            <Input
              v-model="field.input"
              v-bind="field.props"
              :aria-invalid="Boolean(field.errors)"
              id="start-vus"
              aria-describedby="start-vus-error"
              type="number"
              min="0"
              step="1"
            />

            <p
              v-if="field.errors"
              id="start-vus-error"
              class="text-destructive text-xs"
              role="alert"
            >
              {{ field.errors[0] }}
            </p>
          </div>
        </FormischField>

        <FormischFieldArray
          v-slot="fieldArray"
          :of="form"
          :path="['stages']"
        >
          <div class="space-y-4">
            <div class="flex items-center justify-between gap-3">
              <h4 class="text-sm font-medium">Ramp stages</h4>

              <Button
                :disabled="disabled"
                type="button"
                variant="outline"
                size="sm"
                @click="
                  insert(form, {
                    path: ['stages'],
                    initialInput: { duration_ms: 10_000, target_vus: 0 },
                  })
                "
              >
                Add stage
              </Button>
            </div>

            <div
              v-for="(item, index) in fieldArray.items"
              :key="item"
              class="grid items-start gap-3 rounded-md border p-4 sm:grid-cols-[auto_1fr_1fr_auto]"
            >
              <p class="text-muted-foreground pt-2 text-xs">{{ index + 1 }}</p>

              <FormischField
                v-for="{ name, label, min } in stageFields"
                :key="name"
                v-slot="field"
                :of="form"
                :path="['stages', index, name]"
              >
                <div class="space-y-2">
                  <Label :for="`stage-${item}-${name}`">{{ label }}</Label>

                  <Input
                    v-model="field.input"
                    v-bind="field.props"
                    :id="`stage-${item}-${name}`"
                    :min="min"
                    :aria-invalid="Boolean(field.errors)"
                    :aria-describedby="`stage-${item}-${name}-error`"
                    type="number"
                    step="1"
                  />

                  <p
                    v-if="field.errors"
                    :id="`stage-${item}-${name}-error`"
                    class="text-destructive text-xs"
                    role="alert"
                  >
                    {{ field.errors[0] }}
                  </p>
                </div>
              </FormischField>

              <Button
                :disabled="disabled"
                :aria-label="`Remove stage ${index + 1}`"
                type="button"
                variant="ghost"
                class="sm:mt-6"
                @click="remove(form, { path: ['stages'], at: index })"
              >
                Remove
              </Button>
            </div>

            <p
              v-if="fieldArray.errors"
              class="text-destructive text-xs"
              role="alert"
            >
              {{ fieldArray.errors[0] }}
            </p>
          </div>
        </FormischFieldArray>

        <p class="text-muted-foreground text-xs leading-relaxed">
          Each stage ramps linearly from the previous count. Use the same target to hold a level.
          Zero VUs pauses new requests. Retiring users finish their current request and exit; they
          still occupy worker slots until cleanup finishes.
        </p>
      </div>
    </div>
  </FormischField>
</template>
