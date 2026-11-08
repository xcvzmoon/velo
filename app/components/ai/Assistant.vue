<script setup lang="ts">
  import type { AiEvidence, AiProviderConfig, AiTask } from '~/types/ai';
  import type { TestDefinition } from '~/types/contracts';
  import { Field as FormischField, Form as FormischForm, getInput, useForm } from '@formisch/vue';
  import * as v from 'valibot';
  import { aiProviderLabels } from '~/schemas/ai';

  const requestSchema = v.object({
    description: v.pipe(v.string(), v.trim(), v.minLength(1), v.maxLength(16000)),
    includeContext: v.boolean(),
  });
  const props = defineProps<{
    task: AiTask;
    definition?: TestDefinition | null;
    evidence?: AiEvidence | null;
    disabled: boolean;
  }>();
  const emit = defineEmits<{ apply: [definition: TestDefinition] }>();
  const { review, result, error, busy, prepare, generate, reset, cancel } = useAiAssistant();
  const providerConfig = useAiProviderConfig();
  const form = useForm({
    schema: requestSchema,
    initialInput: {
      description:
        props.task === 'explain'
          ? 'Explain these measurements, note gaps, and suggest next investigation steps.'
          : '',
      includeContext: false,
    },
  });
  const inputId = useId();
  const evidenceId = useId();
  const destination = shallowRef<AiProviderConfig | null>(null);
  const providerDirty = shallowRef<boolean>(false);
  const reviewedContent = shallowRef<string>('');
  const title = computed<string>(() =>
    props.task === 'draft' ? 'AI test assistant' : 'AI measurement explanation',
  );

  const reviewedProvider = computed<string>(() =>
    destination.value ? aiProviderLabels[destination.value.provider] : 'Provider',
  );

  async function stage(input: v.InferOutput<typeof requestSchema>): Promise<void> {
    if (props.disabled || providerDirty.value) {
      return;
    }
    reviewedContent.value = '';
    await prepare(
      props.task,
      input.description,
      input.includeContext ? (props.definition ?? null) : null,
      input.includeContext ? (props.evidence ?? null) : null,
      providerConfig.value,
    );
    reviewedContent.value = review.value?.content ?? '';
    destination.value = review.value?.config ?? null;
  }

  watch(() => getInput(form), reset, { deep: true });
  watch(providerConfig, reset, { deep: true });
  watch(providerDirty, (dirty) => {
    if (dirty) {
      reset();
    }
  });
  watch(() => [props.definition, props.evidence, props.disabled], reset);
</script>

<template>
  <section
    class="space-y-5 rounded-lg border p-6"
    :aria-label="title"
  >
    <h2 class="font-medium">{{ title }}</h2>

    <p class="text-muted-foreground text-sm">
      Optional assistance from your selected provider. Configure provider keys in Settings, or use a
      keyless local service. Describe a test, paste a non-sensitive example request or API excerpt,
      or ask for supported assertions and Faker recipes. Nothing is sent until you review the
      payload and explicitly submit it.
    </p>

    <AiProviderConfiguration
      :disabled="disabled || busy"
      @dirty="providerDirty = $event"
    />

    <FormischForm
      :of="form"
      class="space-y-4"
      @submit="stage"
    >
      <FormischField
        v-slot="field"
        :of="form"
        :path="['description']"
      >
        <div class="space-y-2">
          <Label :for="`${inputId}-description`">Instructions and optional API context</Label>

          <Textarea
            v-model="field.input"
            v-bind="field.props"
            :id="`${inputId}-description`"
            :disabled="disabled || busy"
            placeholder="Describe the endpoint, workload goals, assertions, and data you need."
            autocomplete="off"
          />

          <p
            v-if="field.errors"
            class="text-destructive text-sm"
            role="alert"
          >
            {{ field.errors.join(' ') }}
          </p>
        </div>
      </FormischField>

      <FormischField
        v-slot="field"
        :of="form"
        :path="['includeContext']"
      >
        <div class="flex items-center gap-2">
          <Checkbox
            :id="`${inputId}-context`"
            :model-value="field.input"
            :disabled="disabled || busy || (!definition && !evidence)"
            @update:model-value="field.input = $event === true"
          />

          <Label :for="`${inputId}-context`">
            Include
            {{
              task === 'draft' ? 'the current definition structure' : 'the selected measurements'
            }}
            after native redaction
          </Label>
        </div>
      </FormischField>

      <Button
        :disabled="disabled || busy || providerDirty"
        type="submit"
        variant="outline"
      >
        Review provider payload
      </Button>
    </FormischForm>

    <div
      v-if="review || result"
      class="space-y-3"
    >
      <dl
        v-if="destination"
        class="space-y-2 text-sm"
      >
        <div>
          <dt class="text-muted-foreground">Provider and model</dt>
          <dd>{{ reviewedProvider }} · {{ destination.model }}</dd>
        </div>
        <div>
          <dt class="text-muted-foreground">Destination</dt>
          <dd class="font-mono text-xs break-all">{{ destination.endpoint }}</dd>
        </div>
        <div>
          <dt class="text-muted-foreground">Authentication</dt>
          <dd>
            {{ destination.use_key ? 'Stored key for this provider and endpoint' : 'No API key' }}
          </dd>
        </div>
      </dl>

      <p class="text-muted-foreground text-sm">
        Only the payload below and Velo's fixed output instructions are sent to the reviewed
        destination. Request values and string evidence are excluded. Free text can still contain
        private information; inspect it carefully and remove secrets before submission. AI has no
        tools and cannot send target requests or start load.
      </p>

      <ul
        v-if="review?.redactions.length"
        class="list-disc space-y-1 pl-5 text-sm"
        aria-label="Redacted or excluded fields"
      >
        <li
          v-for="(redaction, index) in review.redactions"
          :key="index"
        >
          {{ redaction }}
        </li>
      </ul>

      <pre
        :id="evidenceId"
        tabindex="0"
        class="max-h-96 overflow-auto rounded-md border p-3 text-xs whitespace-pre-wrap"
        >{{ reviewedContent }}</pre>

      <Button
        v-if="review"
        :disabled="disabled || busy || providerDirty"
        type="button"
        @click="generate()"
      >
        Send reviewed payload to {{ reviewedProvider }} ({{ destination?.model }})
      </Button>
    </div>

    <Button
      v-if="busy"
      type="button"
      variant="outline"
      @click="cancel"
    >
      Cancel AI request
    </Button>

    <p
      v-if="busy"
      class="text-muted-foreground text-sm"
      role="status"
    >
      Preparing assistance…
    </p>

    <p
      v-if="error"
      class="text-destructive text-sm"
      role="alert"
    >
      {{ error }}
    </p>

    <p
      v-if="task === 'explain'"
      class="text-muted-foreground text-sm"
    >
      Measurements can be incomplete. Aggregate metrics cannot prove database or network causes.
      Check cited values in the reviewed payload; hypotheses are suggestions to investigate.
    </p>

    <AiResponse
      v-if="result"
      :result="result"
      :previous="definition ?? null"
      :disabled="disabled || busy"
      :evidence-id="evidenceId"
      @apply="emit('apply', $event)"
    />
  </section>
</template>
