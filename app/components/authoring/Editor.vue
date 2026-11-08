<script setup lang="ts">
  import type { FileUploadItem } from '@nuxt/ui';
  import type {
    DataDefinition,
    PreviewResponse,
    RunConfig,
    ScenarioPlan,
    TestDefinition,
  } from '~/types/contracts';
  import { invoke } from '@tauri-apps/api/core';

  const sections = [
    {
      label: 'Request',
      value: 'request',
      slot: 'request',
    },
    {
      label: 'Data',
      value: 'data',
      slot: 'data',
    },
    {
      label: 'Preview & run',
      value: 'preview',
      slot: 'preview',
    },
    {
      label: 'Library & secrets',
      value: 'library',
      slot: 'library',
    },
    {
      label: 'AI assistance',
      value: 'assistant',
      slot: 'assistant',
    },
  ];

  const props = defineProps<{
    disabled: boolean;
    loaded?: TestDefinition | null;
  }>();
  const emit = defineEmits<{
    prepare: [config: RunConfig];
    invalidate: [];
  }>();
  const definition = shallowRef<TestDefinition | null>(null);
  const visual = shallowRef<RunConfig | null>(null);
  const mode = shallowRef<string>('visual');
  const environment = shallowRef<string>('__shared__');
  const preview = shallowRef<PreviewResponse | null>(null);
  const error = shallowRef<string | null>(null);
  const busy = shallowRef<boolean>(false);
  const visualValid = shallowRef<boolean>(true);
  const jsonDirty = shallowRef<boolean>(false);
  const scenarioDirty = shallowRef<boolean>(false);
  const importedData = shallowRef<string>('');
  const dataPreview = shallowRef<Record<string, string>[]>([]);
  const preparedCount = shallowRef<number>(0);
  const dataDirty = shallowRef<boolean>(false);
  let preparation: AbortController | null = null;
  const source = computed<string>(() =>
    definition.value ? JSON.stringify(definition.value, null, 2) : '',
  );
  const environments = computed<string[]>(() => Object.keys(definition.value?.environments ?? {}));
  const draftInvalid = computed<boolean>(
    () =>
      dataDirty.value ||
      scenarioDirty.value ||
      (mode.value === 'json' ? jsonDirty.value : !visualValid.value),
  );
  function update(config: RunConfig): void {
    definition.value = {
      version: 1,

      variables: {},

      environments: {},

      ...definition.value,

      config: {
        ...config,
        scenarios: definition.value?.config.scenarios,
      },
    };
    preview.value = null;
    emitInvalidation();
  }
  function applyScenarios(scenarios: ScenarioPlan | null): void {
    if (!definition.value) {
      return;
    }
    const { scenarios: _previous, ...config } = definition.value.config;
    definition.value = {
      ...definition.value,

      config: scenarios
        ? {
            ...config,
            scenarios,
          }
        : config,
    };
    preview.value = null;
    emitInvalidation();
  }
  function emitInvalidation(): void {
    emit('invalidate');
  }
  function applyDefinition(value: TestDefinition): void {
    definition.value = value;
    visual.value = value.config;
    if (
      !environment.value.startsWith('env:') ||
      !environments.value.includes(environment.value.slice(4))
    ) {
      environment.value = '__shared__';
    }
    jsonDirty.value = false;
    scenarioDirty.value = false;
    importedData.value = '';
    dataPreview.value = [];
    preparedCount.value = 0;
    dataDirty.value = false;
    preview.value = null;
    emitInvalidation();
  }
  async function applyJson(text: string): Promise<void> {
    await perform(async () =>
      applyDefinition(await invoke<TestDefinition>('parse_definition', { source: text })),
    );
  }
  async function perform(action: () => Promise<void>): Promise<void> {
    busy.value = true;
    error.value = null;
    preparation = new AbortController();
    try {
      await action();
    } catch (error_) {
      error.value = error_ instanceof Error ? error_.message : formatAuthoringError(error_);
    } finally {
      preparation = null;
      busy.value = false;
    }
  }
  async function importFile(): Promise<void> {
    await perform(async () => {
      const value = await invoke<TestDefinition | null>('import_definition_file');
      if (value) {
        applyDefinition(value);
      }
    });
  }
  async function exportFile(): Promise<void> {
    await perform(async () => {
      await invoke('export_definition_file', { source: source.value });
    });
  }
  function applyData(dataset: DataDefinition | null): void {
    if (!definition.value) {
      return;
    }
    const { dataset: _previous, ...current } = definition.value;
    definition.value = dataset
      ? {
          ...current,

          dataset,
        }
      : current;
    importedData.value = '';
    dataPreview.value = [];
    preparedCount.value = 0;
    nextTick(() => {
      dataDirty.value = false;
    });
    emitInvalidation();
  }
  async function readDataFile(file: FileUploadItem | null | undefined): Promise<void> {
    importedData.value = '';
    dataPreview.value = [];
    preparedCount.value = 0;
    emitInvalidation();
    if (!(file instanceof File)) {
      return;
    }
    await perform(async () => {
      if (file.size > 8388608) {
        throw new Error('DATASET_BUDGET_EXCEEDED: Import exceeds 8 MiB.');
      }
      let importedSource: string;
      try {
        importedSource = new TextDecoder('utf-8', { fatal: true }).decode(await file.arrayBuffer());
      } catch {
        throw new Error('DATASET_IMPORT_FAILED: Select a readable UTF-8 CSV or JSON file.');
      }
      if (preparation?.signal.aborted) {
        throw new Error('PREPARATION_CANCELLED: Imported data was discarded.');
      }
      importedData.value = importedSource;
    });
  }
  async function prepareRows(): Promise<Record<string, string>[]> {
    if (!definition.value?.dataset) {
      return [];
    }
    const controller = preparation;
    if (!controller) {
      throw new Error('PREPARATION_CANCELLED: Prepared data was discarded.');
    }
    dataPreview.value = [];
    preparedCount.value = 0;
    const rows = await prepareData(definition.value.dataset, importedData.value, controller.signal);
    dataPreview.value = rows.slice(0, 5);
    preparedCount.value = rows.length;
    return rows;
  }
  async function cancelPreparation(): Promise<void> {
    preparation?.abort();
    dataPreview.value = [];
    preparedCount.value = 0;
    try {
      await invoke('cancel_data_preparation');
    } catch {
      error.value = 'PREPARATION_CANCEL_FAILED: Wait for preparation to finish.';
    }
  }
  async function previewData(): Promise<void> {
    await perform(async () => {
      await prepareRows();
    });
  }
  async function review(): Promise<void> {
    await perform(async () => {
      const config = await invoke<RunConfig>('resolve_definition', {
        rows: await prepareRows(),

        definition: definition.value,

        environment: environment.value.startsWith('env:') ? environment.value.slice(4) : null,
      });
      if (preparation?.signal.aborted) {
        throw new Error('PREPARATION_CANCELLED: Prepared data was discarded.');
      }
      emit('prepare', config);
    });
  }
  async function inspect(): Promise<void> {
    preview.value = null;
    await perform(async () => {
      preview.value = await invoke<PreviewResponse>('preview_request', {
        rows: await prepareRows(),

        definition: definition.value,

        environment: environment.value.startsWith('env:') ? environment.value.slice(4) : null,

        authorized: true,
      });
    });
  }
  watch(
    () => props.loaded,
    (value) => {
      if (value) {
        applyDefinition(value);
      }
    },
    { immediate: true },
  );
  watch(
    () => props.disabled,
    (disabled) => {
      if (disabled) {
        preparation?.abort();
        importedData.value = '';
        dataPreview.value = [];
        preparedCount.value = 0;
      }
    },
  );
  onBeforeUnmount(() => {
    if (preparation) {
      void cancelPreparation();
    }
  });
  watch(environment, () => {
    preview.value = null;
    emitInvalidation();
  });
</script>

<template>
  <div class="space-y-6">
    <section
      class="space-y-4 rounded-lg border p-6"
      aria-label="Test definition"
    >
      <h2 class="font-medium">Test definition</h2>

      <p class="text-muted text-sm">
        Visual and JSON editors share a version 1 definition. Changes in JSON apply only after
        parsing succeeds. Use
        <code v-text="'{{name}}'" />
        for variables and
        <code v-text="'{{secret:name}}'" />
        for OS secrets. JSON body placeholders belong inside strings. Exports clear local upload
        paths; select files again after importing.
      </p>

      <div class="flex flex-wrap gap-3">
        <UButton
          :disabled="disabled || busy"
          variant="outline"
          color="neutral"
          label="Import velo.test.json"
          @click="importFile"
        />

        <UButton
          :disabled="disabled || busy || !definition || draftInvalid"
          variant="outline"
          color="neutral"
          label="Export velo.test.json"
          @click="exportFile"
        />
      </div>

      <template v-if="definition">
        <label
          class="text-sm font-medium"
          for="authoring-environment"
        >
          Active environment
        </label>

        <USelect
          v-model="environment"
          :disabled="disabled || busy"
          :items="[
            { label: 'Shared variables only', value: '__shared__' },
            ...environments.map((name) => ({ label: name, value: `env:${name}` })),
          ]"
          id="authoring-environment"
          class="w-full"
        />

        <p class="text-muted text-xs">
          Manage shared variables and named environments in JSON. Environment entries override
          shared values. Put secret references in request fields and keep secret values in the OS
          store.
        </p>
      </template>

      <p
        v-if="error"
        class="text-error text-sm"
        role="alert"
      >
        {{ error }}
      </p>
    </section>

    <UTabs
      :items="sections"
      :unmount-on-hide="false"
      default-value="request"
      variant="link"
      class="gap-6"
    >
      <template #request>
        <div
          class="mb-5 flex flex-wrap gap-2"
          aria-label="Definition editor"
          role="group"
        >
          <UButton
            :disabled="disabled || busy || scenarioDirty"
            :aria-pressed="mode === 'visual'"
            :variant="mode === 'visual' ? 'soft' : 'ghost'"
            label="Visual editor"
            @click="
              visual = definition?.config ?? null;
              mode = 'visual';
            "
          />

          <UButton
            :disabled="disabled || busy || !definition || scenarioDirty"
            :aria-pressed="mode === 'json'"
            :variant="mode === 'json' ? 'soft' : 'ghost'"
            label="JSON and environments"
            @click="mode = 'json'"
          />

          <UButton
            :disabled="disabled || busy || !definition || draftInvalid"
            :aria-pressed="mode === 'scenarios'"
            :variant="mode === 'scenarios' ? 'soft' : 'ghost'"
            label="Scenarios"
            @click="mode = 'scenarios'"
          />
        </div>

        <RunRequestEditor
          v-if="mode === 'visual'"
          :loaded="visual"
          :scenario-mode="Boolean(definition?.config.scenarios)"
          :disabled="disabled || busy"
          @change="update"
          @valid="
            visualValid = $event;
            emitInvalidation();
          "
          @prepare="
            update($event);
            review();
          "
        />

        <AuthoringScenarios
          v-if="mode === 'scenarios' && definition"
          :plan="definition.config.scenarios ?? null"
          :base="definition.config"
          :disabled="disabled || busy"
          @change="applyScenarios"
          @dirty="
            scenarioDirty = $event;
            emitInvalidation();
          "
        />

        <section
          v-if="mode === 'json'"
          class="rounded-lg border p-6"
        >
          <AuthoringTextForm
            :source="source"
            :disabled="disabled || busy"
            label="Version 1 test definition"
            action="Apply JSON"
            @apply="applyJson"
            @dirty="
              jsonDirty = $event;
              emitInvalidation();
            "
          />
        </section>
      </template>

      <template #data>
        <div class="space-y-6">
          <AuthoringDataset
            v-if="definition"
            :disabled="disabled || busy"
            :definition="definition.dataset ?? null"
            @apply="applyData"
            @dirty="
              dataDirty = $event;
              emitInvalidation();
            "
          />

          <section
            v-if="definition?.dataset"
            class="space-y-4 rounded-lg border p-6"
            aria-label="Prepared data preview"
          >
            <h2 class="font-medium">Prepared data preview</h2>

            <p class="text-muted text-sm">
              Preview shows the first five records, including intentional invalid values. Rows stay
              in memory and are discarded when preparation ends. Imported files must be selected
              again after loading a saved test.
            </p>

            <template v-if="definition.dataset.source.kind !== 'faker'">
              <label
                class="text-sm font-medium"
                for="dataset-file"
              >
                CSV or JSON file (up to 8 MiB)
              </label>

              <UFileUpload
                :disabled="disabled || busy"
                :accept="
                  definition.dataset.source.kind === 'csv'
                    ? '.csv,text/csv'
                    : '.json,application/json'
                "
                :model-value="null"
                :preview="false"
                id="dataset-file"
                label="Choose a CSV or JSON file"
                reset
                @update:model-value="readDataFile"
              />
            </template>

            <UButton
              :disabled="disabled || busy || dataDirty"
              variant="outline"
              color="neutral"
              label="Prepare and preview records"
              @click="previewData"
            />

            <UButton
              v-if="busy"
              variant="outline"
              color="neutral"
              label="Cancel data preparation"
              @click="cancelPreparation"
            />

            <p
              v-if="preparedCount"
              class="text-muted text-sm"
            >
              {{ preparedCount }} records prepared. Preview may contain private data.
            </p>

            <pre
              v-if="dataPreview.length"
              class="max-h-80 overflow-auto rounded-md border p-3 text-xs"
              >{{ JSON.stringify(dataPreview, null, 2) }}</pre>
          </section>

          <ShellEmptyState
            v-if="!definition"
            title="Configure a request first"
            description="Your data recipe will bind generated or imported records to the test definition."
          />
        </div>
      </template>

      <template #preview>
        <section
          class="space-y-4 rounded-lg border p-6"
          aria-label="Single request preview"
        >
          <h2 class="font-medium">Preview one request</h2>

          <p class="text-muted text-sm">
            Preview sends one logical request with the configured timeout and redirects. Clicking
            confirms that you are authorized to send it. The response is bounded and known request
            values are redacted. Preview is separate from load-test history.
          </p>

          <div class="flex flex-wrap gap-3">
            <UButton
              :disabled="
                disabled ||
                busy ||
                !definition ||
                draftInvalid ||
                Boolean(definition.config.scenarios)
              "
              variant="outline"
              color="neutral"
              label="I am authorized. Preview request"
              @click="inspect"
            />

            <UButton
              :disabled="disabled || busy || !definition || draftInvalid"
              label="Review load"
              @click="review"
            />
          </div>

          <p
            v-if="definition?.config.scenarios"
            class="text-muted text-sm"
          >
            Single-request preview is unavailable for scenarios. Review load to validate every
            configured step before authorization.
          </p>

          <template v-if="preview">
            <ul
              v-if="preview.checks.length"
              class="space-y-1 text-sm"
              aria-label="Preview assertion outcomes"
            >
              <li
                v-for="(outcome, index) in preview.checks"
                :key="index"
              >
                {{ definition?.config.assertions[index]?.id ?? `Assertion ${index + 1}` }}:
                {{ outcome.replaceAll('_', ' ') }}
              </li>
            </ul>

            <p class="font-mono text-sm">
              HTTP {{ preview.status }} · {{ preview.elapsed_ms.toFixed(1) }} ms
            </p>

            <p class="text-muted text-xs">
              Truncated: {{ preview.truncated ? 'yes' : 'no' }} · Redacted:
              {{ preview.redacted ? 'yes' : 'no' }}
            </p>

            <pre
              class="max-h-64 overflow-auto rounded-md border p-3 text-xs break-all whitespace-pre-wrap"
              >{{ preview.headers.map(({ name, value }) => `${name}: ${value}`).join('\n') }}</pre>

            <pre
              class="max-h-96 overflow-auto rounded-md border p-3 text-xs break-all whitespace-pre-wrap"
              >{{ preview.body }}</pre>
          </template>
        </section>
      </template>

      <template #library>
        <div class="space-y-6">
          <AuthoringSavedTests
            :source="source"
            :disabled="disabled || busy"
            :can-save="definition !== null && !draftInvalid"
            @load="applyDefinition"
          />

          <AuthoringSecrets
            :disabled="disabled || busy"
            @change="
              preview = null;
              emitInvalidation();
            "
          />

          <AuthoringImports
            :disabled="disabled || busy"
            @apply="
              update($event);
              visual = $event;
              mode = 'visual';
            "
          />
        </div>
      </template>

      <template #assistant>
        <AiAssistant
          :definition="definition"
          :disabled="disabled || busy"
          task="draft"
          @apply="applyDefinition"
        />
      </template>
    </UTabs>
  </div>
</template>
