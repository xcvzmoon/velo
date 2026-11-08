import type {
  ApiCollection,
  CollectionEntry,
  FunctionalRun,
  CollectionRequest,
  TestDefinition,
  RunConfig,
} from '~/types/contracts';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

type RequestEntry = Extract<
  CollectionEntry,
  {
    kind: 'request';
  }
>;
type ApiTab = {
  collection_id: string;
  request_id: string;
  instance_id: string;
};
type ApiWorkspaceState = {
  collections: ApiCollection[];
  snapshots: ApiCollection[];
  selected: string | null;
  tabs: ApiTab[];
  active: string | null;
  saved: Record<string, string>;
  error: string | null;
  busy: boolean;
  operation: string | null;
  result: FunctionalRun | null;
};
type ApiRunSettings = {
  environment: string | null;
  iterations: number;
  delayMs: number;
  stopOnFailure: boolean;
  rows: Record<string, string>[];
};
type ApiWorkspace = {
  state: Ref<ApiWorkspaceState>;
  collection: ComputedRef<ApiCollection | null>;
  request: ComputedRef<RequestEntry | null>;
  dirty: ComputedRef<boolean>;
  refresh: () => Promise<void>;
  create: (name: string) => void;
  save: () => Promise<void>;
  remove: () => Promise<void>;
  duplicate: () => void;
  open: (id: string) => void;
  close: (id: string) => void;
  add: (parent: string | null, kind: 'folder' | 'request', name: string) => void;
  change: (id: string, action: 'duplicate' | 'delete' | 'up' | 'down') => void;
  update: (id: string, config: RunConfig) => void;
  perform: (action: () => Promise<void>) => Promise<void>;
  run: (selection: string[], settings: ApiRunSettings, interactive: boolean) => Promise<void>;
  cancel: () => Promise<void>;
  importFile: () => Promise<void>;
  exportFile: () => Promise<void>;
  load: (selection: string[], scenario: boolean) => Promise<void>;
};
/**
 * Owns local API drafts, explicit execution, and collection persistence.
 * Destructive actions require confirmation in the calling UI.
 * @returns Shared in-memory drafts and actions; creating or editing never sends a request.
 * @example
 * ```ts
 * const workspace = useApiWorkspace()
 * await workspace.refresh()
 * ```
 */
export function useApiWorkspace(): ApiWorkspace {
  const confirm = useConfirmation();
  const state = useState<ApiWorkspaceState>('api-workspace', () => ({
    collections: [],
    snapshots: [],
    selected: null,
    tabs: [],
    active: null,
    saved: {},
    error: null,
    busy: false,
    operation: null,
    result: null,
  }));
  const collection = computed<ApiCollection | null>(
    () => state.value.collections.find(({ id }) => id === state.value.selected) ?? null,
  );
  const request = computed<RequestEntry | null>(() => {
    const entry =
      collection.value && state.value.active
        ? findApiEntry(collection.value.entries, state.value.active)
        : null;
    return entry?.kind === 'request' ? entry : null;
  });
  const dirty = computed<boolean>(() =>
    state.value.collections.some((item) => JSON.stringify(item) !== state.value.saved[item.id]),
  );
  async function perform(action: () => Promise<void>): Promise<void> {
    state.value.error = null;
    try {
      await action();
    } catch (error) {
      state.value.error = formatAuthoringError(error);
    }
  }
  async function refresh(): Promise<void> {
    await perform(async () => {
      const loaded = await invoke<ApiCollection[]>('list_api_collections');
      for (const item of loaded) {
        const existing = state.value.collections.find(({ id }) => id === item.id);
        if (!existing) {
          state.value.snapshots.push(structuredClone(item));
          state.value.collections.push(item);
          state.value.saved[item.id] = JSON.stringify(item);
        }
      }
      state.value.selected ??= loaded[0]?.id ?? null;
    });
  }
  function create(name: string): void {
    const item = newApiCollection(name);
    state.value.collections.push(item);
    state.value.selected = item.id;
    state.value.active = null;
  }
  async function save(): Promise<void> {
    const current = collection.value;
    if (!current) {
      return;
    }
    await perform(async () => {
      await invoke('save_api_collection', { collection: current });
      state.value.saved[current.id] = JSON.stringify(current);
      const position = state.value.snapshots.findIndex(({ id }) => id === current.id);
      const snapshot = structuredClone(toRaw(current));
      if (position < 0) {
        state.value.snapshots.push(snapshot);
      } else {
        state.value.snapshots.splice(position, 1, snapshot);
      }
    });
  }
  async function remove(): Promise<void> {
    const current = collection.value;
    if (!current) {
      return;
    }
    await perform(async () => {
      await invoke('delete_api_collection', { id: current.id });
      state.value.collections = state.value.collections.filter(({ id }) => id !== current.id);
      state.value.tabs = state.value.tabs.filter(
        ({ collection_id }) => collection_id !== current.id,
      );
      state.value.selected = state.value.collections[0]?.id ?? null;
      state.value.active = null;
    });
  }
  function duplicate(): void {
    const current = collection.value;
    if (!current) {
      return;
    }
    const copy = structuredClone(toRaw(current));
    copy.id = crypto.randomUUID();
    copy.name += ' copy';
    state.value.collections.push(copy);
    state.value.selected = copy.id;
    state.value.active = null;
  }
  function open(id: string): void {
    if (!collection.value) {
      return;
    }
    state.value.active = id;
    if (
      !state.value.tabs.some(
        (item) => item.request_id === id && item.collection_id === collection.value?.id,
      )
    ) {
      state.value.tabs.push({
        collection_id: collection.value.id,
        request_id: id,
        instance_id: crypto.randomUUID(),
      });
    }
  }
  function close(id: string): void {
    state.value.tabs = state.value.tabs.filter(
      (tab) => tab.request_id !== id || tab.collection_id !== collection.value?.id,
    );
    if (state.value.active === id) {
      state.value.active =
        state.value.tabs.findLast((tab) => tab.collection_id === collection.value?.id)
          ?.request_id ?? null;
    }
  }
  function add(parent: string | null, kind: 'folder' | 'request', name: string): void {
    const current = collection.value;
    if (!current) {
      return;
    }
    const target = parent ? findApiEntry(current.entries, parent) : null;
    const entries = target?.kind === 'folder' ? target.entries : current.entries;
    const entry: CollectionEntry =
      kind === 'request'
        ? newApiRequest(name)
        : {
            kind: 'folder',
            id: crypto.randomUUID(),
            name,
            description: '',
            notes: '',
            defaults: {
              headers: [],
              auth: null,
              variables: {},
            },
            entries: [],
          };
    entries.push(entry);
    if (entry.kind === 'request') {
      open(entry.id);
    }
  }
  function change(id: string, action: 'duplicate' | 'delete' | 'up' | 'down'): void {
    if (!collection.value) {
      return;
    }
    changeApiEntry(collection.value.entries, id, action);
    if (action === 'delete') {
      state.value.tabs = state.value.tabs.filter(
        (tab) =>
          tab.collection_id !== collection.value?.id ||
          Boolean(findApiEntry(collection.value.entries, tab.request_id)),
      );
    }
  }
  function update(id: string, config: RunConfig): void {
    const entry = collection.value ? findApiEntry(collection.value.entries, id) : null;
    if (entry?.kind === 'request') {
      entry.definition.config = config;
    }
  }
  async function run(
    selection: string[],
    settings: ApiRunSettings,
    interactive: boolean,
  ): Promise<void> {
    const current = collection.value;
    if (!current || state.value.busy) {
      return;
    }
    state.value.busy = true;
    state.value.result = null;
    const operationId = crypto.randomUUID();
    state.value.operation = operationId;
    let unlisten: (() => void) | null = null;
    await perform(async () => {
      try {
        unlisten = await listen<FunctionalRun>('api-progress', ({ payload }) => {
          if (payload.id === operationId) {
            state.value.result = payload;
          }
        });
        state.value.result = await invoke<FunctionalRun>('run_api_collection', {
          operationId,
          collection: current,
          selection,
          ...settings,
          interactive,
          authorized: true,
        });
      } finally {
        unlisten?.();
        state.value.busy = false;
        state.value.operation = null;
      }
    });
  }
  async function cancel(): Promise<void> {
    await perform(async () => {
      if (state.value.operation) {
        await invoke('cancel_api_run', { operationId: state.value.operation });
      }
    });
  }
  async function importFile(): Promise<void> {
    await perform(async () => {
      const item = await invoke<ApiCollection | null>('import_api_collection_file');
      if (item) {
        item.id = crypto.randomUUID();
        state.value.collections.push(item);
        state.value.selected = item.id;
        state.value.active = null;
      }
    });
  }
  async function exportFile(): Promise<void> {
    await perform(async () => {
      if (collection.value) {
        await invoke('export_api_collection_file', { collection: collection.value });
      }
    });
  }
  async function load(selection: string[], scenario: boolean): Promise<void> {
    await perform(async () => {
      const current = collection.value;
      if (!current) {
        return;
      }
      const definition = scenario
        ? await invoke<TestDefinition>('collection_scenario', {
            collection: current,
            selection,
          })
        : (
            await invoke<CollectionRequest>('collection_request', {
              collection: current,
              id: selection[0] ?? '',
            })
          ).definition;
      if (
        scenario &&
        !(await confirm(
          'Review as a load scenario?',
          'Review workload, transport, extraction, and failure settings in Test studio before starting load.',
        ))
      ) {
        return;
      }
      useState<TestDefinition | null>('api-load-definition', () => null).value = definition;
      await navigateTo('/tests');
    });
  }
  return {
    state,
    collection,
    request,
    dirty,
    refresh,
    create,
    save,
    remove,
    duplicate,
    open,
    close,
    add,
    change,
    update,
    perform,
    run,
    cancel,
    importFile,
    exportFile,
    load,
  };
}
