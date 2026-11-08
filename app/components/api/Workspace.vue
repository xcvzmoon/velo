<script setup lang="ts">
  import type {
    ApiCollection,
    CollectionRequest,
    CollectionEntry,
    DataDefinition,
    RunConfig,
  } from '~/types/contracts';
  import { Field, Form, getInput, useForm } from '@formisch/vue';
  import { invoke } from '@tauri-apps/api/core';
  import * as v from 'valibot';

  const sections = [
    {
      label: 'Requests',
      slot: 'requests',
    },
    {
      label: 'Run collection',
      slot: 'runner',
    },
    {
      label: 'History',
      slot: 'history',
    },
    {
      label: 'Secrets',
      slot: 'secrets',
    },
  ];

  const workspace = useApiWorkspace();
  const { state, collection, request, dirty } = workspace;
  const searchForm = useForm({
    schema: v.object({ search: v.string() }),
    initialInput: { search: '' },
  });
  const creation = shallowRef<{
    parent: string | null;
    kind: 'collection' | 'folder' | 'request';
  }>({
    parent: null,
    kind: 'collection',
  });
  const selection = ref<string[]>([]);
  const environment = shallowRef<string | null>(null);
  const rowsByTab = ref<Record<string, Record<string, string>[]>>({});
  const rows = computed<Record<string, string>[]>(() => rowsByTab.value[activeKey.value] ?? []);
  const preview = shallowRef<RunConfig | null>(null);
  const documentMode = shallowRef<boolean>(false);
  const documentSource = shallowRef<string>('');
  const documentDirty = shallowRef<boolean>(false);
  const invalid = ref<Record<string, boolean>>({});
  const rawDirty = ref<Record<string, boolean>>({});
  const environments = computed<string[]>(() => Object.keys(collection.value?.environments ?? {}));
  const tabs = computed<
    {
      id: string;
      entry: CollectionEntry;
      dirty: boolean;
    }[]
  >(() => {
    const result: {
      id: string;
      entry: CollectionEntry;
      dirty: boolean;
    }[] = [];
    const saved = state.value.snapshots.find((item) => item.id === collection.value?.id);
    for (const tab of state.value.tabs) {
      if (tab.collection_id !== collection.value?.id) {
        continue;
      }
      const entry = findApiEntry(collection.value.entries, tab.request_id);
      if (entry) {
        result.push({
          id: tab.request_id,
          entry,
          dirty:
            !saved ||
            JSON.stringify(entry) !== JSON.stringify(findApiEntry(saved.entries, tab.request_id)),
        });
      }
    }
    return result;
  });
  const hasUnsaved = computed<boolean>(
    () =>
      dirty.value ||
      documentDirty.value ||
      Object.values(rawDirty.value).some(Boolean) ||
      Object.values(invalid.value).some(Boolean),
  );
  const blocked = computed<boolean>(
    () => state.value.busy || documentDirty.value || Object.values(invalid.value).some(Boolean),
  );
  const scope = computed<string>(() => {
    if (!selection.value.length) {
      return 'Entire collection in displayed order';
    }
    return selection.value
      .map((id) =>
        collection.value ? (findApiEntry(collection.value.entries, id)?.name ?? id) : id,
      )
      .join(', ');
  });
  const activeKey = computed<string>(
    () =>
      state.value.tabs.find(
        (tab) => tab.collection_id === collection.value?.id && tab.request_id === request.value?.id,
      )?.instance_id ?? '',
  );
  const summary = computed<{
    passed: number;
    failed: number;
    cancelled: number;
    skipped: number;
    checksPassed: number;
    checksFailed: number;
  }>(() => {
    const count = {
      passed: 0,
      failed: 0,
      cancelled: 0,
      skipped: 0,
      checksPassed: 0,
      checksFailed: 0,
    };
    for (const result of state.value.result?.results ?? []) {
      count[result.state]++;
      for (const check of result.response?.checks ?? []) {
        if (check === 'passed') {
          count.checksPassed++;
        } else {
          count.checksFailed++;
        }
      }
    }
    return count;
  });
  function closeTab(id: string): void {
    const tab = state.value.tabs.find(
      (entry) => entry.collection_id === collection.value?.id && entry.request_id === id,
    );
    if (!tab) {
      return;
    }
    if (
      (dirty.value ||
        rawDirty.value[tab.instance_id] ||
        invalid.value[tab.instance_id] ||
        invalid.value[`${tab.instance_id}:data`]) &&
      !window.confirm(
        'Close this tab? Valid changes remain in the collection draft; unapplied form edits will be discarded.',
      )
    ) {
      return;
    }
    invalid.value[tab.instance_id] = false;
    invalid.value[`${tab.instance_id}:data`] = false;
    rawDirty.value[tab.instance_id] = false;
    workspace.close(id);
  }
  function create(name: string): void {
    if (documentDirty.value && !window.confirm('Discard unapplied collection JSON edits?')) {
      return;
    }
    documentDirty.value = false;
    documentMode.value = false;
    const target = creation.value;
    if (target.kind === 'collection') {
      workspace.create(name);
    } else {
      workspace.add(target.parent, target.kind, name);
    }
  }
  function add(parent: string | null, kind: 'folder' | 'request'): void {
    creation.value = { parent, kind };
  }
  function changeSelection(id: string): void {
    selection.value = selection.value.includes(id)
      ? selection.value.filter((item) => item !== id)
      : [...selection.value, id];
  }
  function switchCollection(id: string): void {
    if (documentDirty.value && !window.confirm('Discard unapplied collection JSON edits?')) {
      return;
    }
    state.value.selected = id;
    state.value.active = null;
    selection.value = [];
    preview.value = null;
    documentMode.value = false;
    documentDirty.value = false;
  }
  function metadata(
    collectionId: string,
    id: string,
    name: string,
    description: string,
    notes: string,
    inheritAuth: boolean,
  ): void {
    const owner = state.value.collections.find((item) => item.id === collectionId);
    const entry = owner ? findApiEntry(owner.entries, id) : null;
    if (entry?.kind !== 'request') {
      return;
    }
    entry.name = name;
    entry.description = description;
    entry.notes = notes;
    entry.inherit_auth = inheritAuth;
  }
  function updateRequest(collectionId: string, id: string, config: RunConfig): void {
    const owner = state.value.collections.find((item) => item.id === collectionId);
    const entry = owner ? findApiEntry(owner.entries, id) : null;
    if (entry?.kind === 'request') {
      entry.definition.config = config;
    }
    preview.value = null;
  }
  function editDocument(): void {
    if (!collection.value) {
      return;
    }
    documentSource.value = JSON.stringify(collection.value, null, 2);
    documentMode.value = true;
  }
  function closeDocument(): void {
    if (documentDirty.value && !window.confirm('Discard unapplied collection JSON edits?')) {
      return;
    }
    documentDirty.value = false;
    documentMode.value = false;
  }
  async function applyDocument(source: string): Promise<void> {
    await workspace.perform(async () => {
      const parsed = await invoke<ApiCollection>('parse_api_collection', { source });
      const index = state.value.collections.findIndex(({ id }) => id === collection.value?.id);
      if (index < 0) {
        return;
      }
      if (parsed.id !== collection.value?.id) {
        throw new Error('COLLECTION_ID_CHANGED: Keep the current collection ID when editing JSON.');
      }
      state.value.collections.splice(index, 1, parsed);
      for (const tab of state.value.tabs) {
        if (tab.collection_id === parsed.id) {
          invalid.value[tab.instance_id] = false;
          invalid.value[`${tab.instance_id}:data`] = false;
          rawDirty.value[tab.instance_id] = false;
        }
      }
      state.value.tabs = state.value.tabs.filter((tab) => tab.collection_id !== parsed.id);
      state.value.active = null;
      documentDirty.value = false;
      documentMode.value = false;
    });
  }
  async function resolvedPreview(): Promise<void> {
    await workspace.perform(async () => {
      if (!collection.value || !request.value) {
        return;
      }
      preview.value = await invoke<RunConfig>('preview_api_request', {
        collection: collection.value,
        requestId: request.value.id,
        environment: environment.value,
        rows: rows.value,
      });
    });
  }
  async function run(
    settings: {
      environment: string | null;
      iterations: number;
      delayMs: number;
      stopOnFailure: boolean;
      rows: Record<string, string>[];
    },
    interactive: boolean,
  ): Promise<void> {
    environment.value = settings.environment;
    await workspace.run(
      interactive && request.value ? [request.value.id] : selection.value,
      settings,
      interactive,
    );
  }
  function applyData(dataset: DataDefinition | null): void {
    if (!request.value) {
      return;
    }
    const { dataset: _previous, ...definition } = request.value.definition;
    rowsByTab.value[activeKey.value] = [];
    request.value.definition = dataset
      ? {
          ...definition,
          dataset,
        }
      : definition;
  }
  async function save(): Promise<void> {
    await workspace.save();
    if (!state.value.error) {
      for (const tab of state.value.tabs) {
        if (tab.collection_id === collection.value?.id) {
          rawDirty.value[tab.instance_id] = false;
        }
      }
    }
  }
  async function reorder(direction: -1 | 1): Promise<void> {
    const items = state.value.collections;
    const index = items.findIndex(({ id }) => id === state.value.selected);
    const item = items[index];
    const destination = index + direction;
    if (!item || destination < 0 || destination >= items.length) {
      return;
    }
    const ordered = items.map(({ id }) => id);
    ordered.splice(index, 1);
    ordered.splice(destination, 0, item.id);
    await workspace.perform(async () => {
      await invoke('reorder_api_collections', { ids: ordered });
      items.splice(index, 1);
      items.splice(destination, 0, item);
    });
  }
  function restore(snapshot: CollectionRequest): void {
    workspace.create(`${snapshot.name.slice(0, 130)} history`);
    const entry = newApiRequest(snapshot.name);
    if (entry.kind !== 'request' || !collection.value) {
      return;
    }
    entry.definition = structuredClone(toRaw(snapshot.definition));
    entry.extractions = snapshot.extractions;
    entry.inherit_auth = false;
    collection.value.entries.push(entry);
    workspace.open(entry.id);
  }
  function exportEvidence(): void {
    if (!state.value.result) {
      return;
    }
    const url = URL.createObjectURL(
      new Blob([JSON.stringify(state.value.result, null, 2)], { type: 'application/json' }),
    );
    try {
      const link = document.createElement('a');
      link.href = url;
      link.download = 'velo-functional-run.json';
      link.click();
    } finally {
      URL.revokeObjectURL(url);
    }
  }
  watch(
    () => state.value.tabs.map((tab) => tab.instance_id).join(','),
    () => {
      const open = new Set(state.value.tabs.map((tab) => tab.instance_id));
      for (const key of Object.keys(invalid.value)) {
        if (!open.has(key.endsWith(':data') ? key.slice(0, -5) : key)) {
          invalid.value[key] = false;
        }
      }
      for (const key of Object.keys(rawDirty.value)) {
        if (!open.has(key)) {
          rawDirty.value[key] = false;
        }
      }
    },
  );
  watch(activeKey, () => {
    preview.value = null;
  });
  useEventListener(window, 'beforeunload', (event) => {
    if (hasUnsaved.value) {
      event.preventDefault();
      event.returnValue = '';
    }
  });
  watch(
    hasUnsaved,
    (unsaved) => {
      void invoke('api_draft_state', { dirty: unsaved }).catch((error) => {
        state.value.error = formatAuthoringError(error);
      });
    },
    { immediate: true },
  );
  onMounted(workspace.refresh);
</script>

<template>
  <div class="space-y-6">
    <p
      v-if="state.error"
      class="text-error text-sm"
      role="alert"
    >
      {{ state.error }}
    </p>

    <div class="grid items-start gap-5 xl:grid-cols-[18rem_minmax(0,1fr)]">
      <section
        class="space-y-4 rounded-lg border p-5"
        aria-label="API collections"
      >
        <div class="flex flex-wrap gap-2">
          <UButton
            :disabled="state.busy"
            variant="outline"
            color="neutral"
            label="New collection"
            @click="creation = { parent: null, kind: 'collection' }"
          />

          <UButton
            :disabled="state.busy"
            variant="outline"
            color="neutral"
            label="Import collection"
            @click="workspace.importFile"
          />

          <UButton
            :disabled="state.busy"
            variant="outline"
            color="neutral"
            label="Refresh local collections"
            @click="workspace.refresh"
          />
        </div>

        <ApiNameForm
          :label="`New ${creation.kind} name`"
          :disabled="state.busy"
          @submit="create"
        />

        <div
          class="flex flex-wrap gap-2"
          aria-label="Collections"
          role="group"
        >
          <UButton
            v-for="item in state.collections"
            :key="item.id"
            :disabled="state.busy"
            :variant="item.id === state.selected ? 'soft' : 'ghost'"
            :label="`${item.name}${JSON.stringify(item) !== state.saved[item.id] ? ' •' : ''}`"
            @click="switchCollection(item.id)"
          />
        </div>

        <template v-if="collection">
          <div class="flex flex-wrap gap-2">
            <UButton
              :disabled="blocked"
              :label="`Save collection${dirty ? ' •' : ''}`"
              @click="save"
            />

            <UButton
              :disabled="state.busy || documentDirty"
              variant="outline"
              color="neutral"
              label="Duplicate collection"
              @click="workspace.duplicate"
            />

            <UButton
              :disabled="blocked"
              variant="outline"
              color="neutral"
              label="Export collection"
              @click="workspace.exportFile"
            />

            <UButton
              :disabled="state.busy"
              variant="outline"
              color="neutral"
              label="Move collection up"
              @click="reorder(-1)"
            />

            <UButton
              :disabled="state.busy"
              variant="outline"
              color="neutral"
              label="Move collection down"
              @click="reorder(1)"
            />

            <ShellConfirmAction
              :disabled="state.busy || documentDirty"
              label="Delete collection"
              title="Delete collection?"
              description="This removes the collection and every request it contains."
              @confirm="workspace.remove"
            />
          </div>

          <p class="text-sm">{{ collection.description }}</p>
          <p class="text-muted text-xs whitespace-pre-wrap">{{ collection.notes }}</p>

          <div class="flex flex-wrap gap-2">
            <UButton
              :disabled="state.busy"
              variant="outline"
              color="neutral"
              label="New request"
              @click="add(null, 'request')"
            />

            <UButton
              :disabled="state.busy"
              variant="outline"
              color="neutral"
              label="New folder"
              @click="add(null, 'folder')"
            />

            <UButton
              :disabled="state.busy || documentDirty"
              variant="outline"
              color="neutral"
              label="Edit collection JSON, defaults, environments, and extractions"
              @click="editDocument"
            />

            <UButton
              :disabled="blocked"
              variant="outline"
              color="neutral"
              label="Select entire collection"
              @click="selection = []"
            />

            <UButton
              :disabled="blocked"
              variant="outline"
              color="neutral"
              label="Review as load scenario"
              @click="workspace.load(selection, true)"
            />
          </div>

          <Form
            :of="searchForm"
            @submit="() => {}"
          >
            <Field
              v-slot="field"
              :of="searchForm"
              :path="['search']"
            >
              <div class="space-y-2">
                <label
                  class="text-sm font-medium"
                  for="api-collection-search"
                >
                  Search requests and folders
                </label>

                <UInput
                  v-model="field.input"
                  v-bind="field.props"
                  id="api-collection-search"
                  type="search"
                />
              </div>
            </Field>
          </Form>

          <ApiCollectionTree
            :entries="collection.entries"
            :selected-ids="selection"
            :active="state.active"
            :query="getInput(searchForm).search ?? ''"
            :disabled="state.busy || documentMode"
            @open="workspace.open"
            @change="workspace.change"
            @add="add"
            @run="changeSelection"
          />
        </template>
      </section>

      <UTabs
        :items="sections"
        :unmount-on-hide="false"
        variant="link"
        class="min-w-0 gap-5"
      >
        <template #requests>
          <div class="space-y-5">
            <section
              v-if="documentMode"
              class="space-y-4 rounded-lg border p-5"
            >
              <p class="text-muted text-sm">
                Edit names, descriptions, usage notes, folder defaults, variables, environments,
                response JSON pointers to redact, assertions, and extraction rules. Apply validates
                the complete versioned document. Secret values belong in the OS credential store.
                Request and folder IDs must remain unique.
              </p>

              <AuthoringTextForm
                :source="documentSource"
                :disabled="state.busy"
                label="Version 1 API collection"
                action="Apply collection JSON"
                @apply="applyDocument"
                @dirty="documentDirty = $event"
              />

              <UButton
                variant="outline"
                color="neutral"
                label="Back to requests"
                @click="closeDocument"
              />
            </section>

            <section
              v-show="!documentMode"
              class="space-y-4"
              aria-label="Request tabs"
            >
              <div class="flex flex-wrap gap-2">
                <div
                  v-for="tab in tabs"
                  :key="tab.id"
                  class="flex gap-1"
                >
                  <UButton
                    :disabled="state.busy"
                    :variant="tab.id === state.active ? 'soft' : 'ghost'"
                    :label="`${tab.entry.name}${tab.dirty || rawDirty[state.tabs.find((item) => item.request_id === tab.id)?.instance_id ?? ''] ? ' •' : ''}`"
                    @click="workspace.open(tab.id)"
                  />

                  <UButton
                    :disabled="state.busy"
                    :aria-label="`Close ${tab.entry.name}`"
                    variant="ghost"
                    color="neutral"
                    label="×"
                    @click="closeTab(tab.id)"
                  />
                </div>
              </div>

              <KeepAlive>
                <ApiRequestTab
                  v-if="request"
                  :key="activeKey"
                  :entry="request"
                  :collection-id="collection?.id ?? ''"
                  :tab-key="activeKey"
                  :disabled="state.busy"
                  @change="updateRequest"
                  @metadata="metadata"
                  @valid="(key, value) => (invalid[key] = !value)"
                  @dirty="(key, value) => (rawDirty[key] = value)"
                />
              </KeepAlive>

              <template v-if="request">
                <div class="flex flex-wrap gap-3">
                  <UButton
                    :disabled="blocked"
                    variant="outline"
                    color="neutral"
                    label="Preview resolved request (masked)"
                    @click="resolvedPreview"
                  />

                  <UButton
                    :disabled="blocked"
                    variant="outline"
                    color="neutral"
                    label="Open in load-test editor"
                    @click="workspace.load([request.id], false)"
                  />
                </div>

                <pre
                  v-if="preview"
                  class="bg-muted max-h-80 overflow-auto rounded-md p-4 font-mono text-xs break-all whitespace-pre-wrap"
                  >{{ JSON.stringify(preview, null, 2) }}</pre>

                <KeepAlive>
                  <ApiData
                    :key="activeKey"
                    :definition="request.definition.dataset ?? null"
                    :disabled="state.busy"
                    @apply="applyData"
                    @rows="rowsByTab[activeKey] = $event"
                    @dirty="invalid[`${activeKey}:data`] = $event"
                  />
                </KeepAlive>
              </template>
            </section>

            <ShellEmptyState
              v-if="!request && !documentMode"
              title="Select a request"
              description="Open a request from your collection, or create one to start authoring."
            />
          </div>
        </template>

        <template #runner>
          <div class="space-y-5">
            <ApiRunnerForm
              v-if="collection"
              :environments="environments"
              :disabled="blocked"
              :has-request="Boolean(request)"
              :selection="scope"
              :prepared-rows="rows"
              @run="run"
              @environment="
                environment = $event;
                preview = null;
              "
            />

            <UButton
              v-if="state.busy"
              color="error"
              label="Cancel API execution"
              @click="workspace.cancel"
            />

            <ApiSession
              v-if="collection"
              :collection-id="collection.id"
              :environment="environment"
              :disabled="state.busy"
              :revision="state.result?.id ?? null"
            />

            <section
              v-if="state.result"
              class="space-y-4 rounded-lg border p-5"
              aria-label="Functional run evidence"
            >
              <h2 class="font-medium">Functional run · {{ state.result.state }}</h2>

              <p
                class="text-sm"
                aria-live="polite"
              >
                {{ state.result.results.length }} / {{ state.result.total }} request outcomes
                recorded. {{ summary.passed }} passed, {{ summary.failed }} failed,
                {{ summary.cancelled }} cancelled, {{ summary.skipped }} skipped. Assertions:
                {{ summary.checksPassed }} passed, {{ summary.checksFailed }} failed or unavailable.
              </p>

              <p
                v-if="state.result.storage_error"
                class="text-error text-sm"
                role="alert"
              >
                History could not be saved: {{ state.result.storage_error }}
              </p>

              <UButton
                variant="outline"
                color="neutral"
                label="Export redacted functional evidence"
                @click="exportEvidence"
              />

              <details
                v-for="(result, index) in state.result.results"
                :key="index"
                :open="state.result.results.length === 1"
                class="space-y-3 border-t pt-3"
              >
                <summary class="cursor-pointer text-sm">
                  Iteration {{ result.iteration }} · {{ result.name }} · {{ result.state }}
                </summary>

                <p
                  v-if="result.error"
                  class="text-error text-xs"
                  role="alert"
                >
                  {{ result.error }}
                </p>

                <ApiResponse
                  v-if="result.response"
                  :response="result.response"
                  :assertions="
                    state.result.requests.find((request) => request.id === result.request_id)
                      ?.definition.config.assertions
                  "
                />
              </details>
            </section>

            <ShellEmptyState
              v-if="!collection"
              title="Choose a collection"
              description="Create or select a collection before reviewing functional execution."
            />
          </div>
        </template>

        <template #history>
          <ApiHistory
            :disabled="state.busy"
            :revision="state.result?.state === 'running' ? null : (state.result?.id ?? null)"
            @restore="restore"
            @inspect="state.result = $event"
          />
        </template>

        <template #secrets>
          <AuthoringSecrets :disabled="state.busy" />
        </template>
      </UTabs>
    </div>
  </div>
</template>
