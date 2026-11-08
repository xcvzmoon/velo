<script setup lang="ts">
  import { invoke } from '@tauri-apps/api/core';

  const revision = shallowRef<number>(0);
  const baseline = shallowRef<string | null>(null);
  const entries = shallowRef<HistoryEntry[]>([]);
  const selected = shallowRef<HistoryEntry[]>([]);
  const error = shallowRef<string | null>(null);
  const status = shallowRef<string | null>(null);
  const busy = shallowRef<boolean>(false);
  const cursor = shallowRef<string | null>(null);

  const candidate = computed<HistoryEntry | null>(() => selected.value.at(-1) ?? null);

  async function refreshStatus(): Promise<void> {
    try {
      status.value = await invoke<string | null>('history_status');
    } catch {
      status.value = 'HISTORY_STATUS_UNAVAILABLE';
    }
  }

  async function refresh(next = false): Promise<void> {
    busy.value = true;
    error.value = null;
    try {
      const page = await invoke<HistoryEntry[]>('list_history', {
        before: next ? cursor.value : null,
      });
      entries.value = page;
      baseline.value = await invoke<string | null>('pinned_baseline');
      selected.value = await Promise.all(
        selected.value.map(({ id }) => invoke<HistoryEntry>('load_history', { id })),
      );
      cursor.value = page.at(-1)?.id ?? null;
      revision.value++;
      await refreshStatus();
    } catch {
      error.value = 'HISTORY_UNAVAILABLE: Local history could not be loaded.';
    } finally {
      busy.value = false;
    }
  }

  async function select(id: string): Promise<void> {
    busy.value = true;
    error.value = null;
    try {
      if (selected.value.some((entry) => entry.id === id)) {
        selected.value = selected.value.filter((entry) => entry.id !== id);
        return;
      }
      if (selected.value.length >= 2) {
        error.value = 'Select at most two runs. Remove a selection first.';
        return;
      }
      const entry = await invoke<HistoryEntry>('load_history', { id });
      const first = selected.value[0]?.snapshot.confirmation;
      const current = entry.snapshot.confirmation;
      if (
        first &&
        (first.origin !== current.origin ||
          selected.value[0]?.method !== entry.method ||
          first.workload.kind !== current.workload.kind)
      ) {
        error.value =
          'Select runs with the same origin, HTTP method, and workload kind for comparison.';
        return;
      }
      selected.value = [...selected.value, entry];
    } catch {
      error.value = 'HISTORY_UNAVAILABLE: The selected summary could not be loaded.';
    } finally {
      busy.value = false;
    }
  }

  async function pin(id: string | null): Promise<void> {
    busy.value = true;
    error.value = null;
    try {
      await invoke('pin_baseline', { id });
      baseline.value = id;
    } catch {
      error.value = 'BASELINE_SAVE_FAILED: The baseline could not be changed.';
    } finally {
      busy.value = false;
    }
  }

  function updated(entry: HistoryEntry): void {
    selected.value = selected.value.map((item) => (item.id === entry.id ? entry : item));
    entries.value = entries.value.map((item) => (item.id === entry.id ? entry : item));
  }

  async function remove(id: string): Promise<void> {
    busy.value = true;
    error.value = null;
    try {
      await invoke('delete_history', { id });
      selected.value = selected.value.filter((entry) => entry.id !== id);
      await refresh();
    } catch {
      error.value = 'DELETE_FAILED: The history entry could not be removed.';
    } finally {
      busy.value = false;
    }
  }

  useIntervalFn(() => {
    void refreshStatus();
  }, 2000);

  onMounted(() => refresh());
</script>

<template>
  <ShellPanel
    variant="section"
    aria-label="Run history"
  >
    <template #header>
      <div class="flex items-center justify-between gap-4">
        <h2 class="text-highlighted font-semibold">Run history</h2>

        <UButton
          :disabled="busy"
          variant="outline"
          color="neutral"
          label="Refresh latest"
          @click="refresh()"
        />
      </div>
    </template>

    <p class="text-muted text-sm">
      Summaries remain until deletion. Select up to two runs to compare observed results. Matching
      origin, method, and workload does not guarantee identical request data or target conditions.
    </p>

    <p
      v-if="error || status"
      class="text-error text-sm"
      role="alert"
    >
      {{
        error ||
        `${status}: History may be incomplete. Current execution results remain available above.`
      }}
    </p>

    <USkeleton
      v-if="busy"
      class="h-16 w-full"
      aria-label="Loading run history"
      role="status"
    />

    <p
      v-if="!busy && !entries.length"
      class="text-muted text-sm"
    >
      No stored runs on this page.
    </p>

    <div class="flex flex-wrap items-center gap-3 text-sm">
      <p>
        {{
          baseline
            ? `Pinned baseline: ${baseline}. If deleted, dependent comparisons remain unavailable.`
            : 'No pinned baseline.'
        }}
      </p>

      <UButton
        v-if="baseline"
        :disabled="busy"
        variant="outline"
        color="neutral"
        label="Remove baseline"
        @click="pin(null)"
      />
    </div>

    <ul class="space-y-3">
      <li
        v-for="entry in entries"
        :key="entry.id"
        class="border-default flex flex-wrap items-center gap-3 border-b pb-3 text-sm"
      >
        <span class="flex-1">
          {{ new Date(Number(entry.created_at) * 1000).toLocaleString() }} · {{ entry.method }}
          {{ entry.snapshot.confirmation.origin }} ·
          {{ entry.interrupted ? 'interrupted' : entry.snapshot.state }}
          {{ entry.incomplete ? ' · incomplete history' : '' }}
          {{ entry.metadata.release ? ` · ${entry.metadata.release}` : '' }}
          {{ entry.metadata.git_sha ? ` · ${entry.metadata.git_sha}` : '' }}
          {{ entry.metadata.tags.length ? ` · ${entry.metadata.tags.join(', ')}` : '' }}
        </span>

        <UButton
          :disabled="
            busy ||
            baseline === entry.id ||
            (['starting', 'running', 'stopping'].includes(entry.snapshot.state) &&
              !entry.interrupted)
          "
          :label="baseline === entry.id ? 'Baseline' : 'Pin baseline'"
          variant="outline"
          color="neutral"
          @click="pin(entry.id)"
        />

        <UButton
          :disabled="busy"
          :aria-pressed="selected.some((item) => item.id === entry.id)"
          :label="selected.some((item) => item.id === entry.id) ? 'Remove selection' : 'Select'"
          variant="outline"
          color="neutral"
          @click="select(entry.id)"
        />

        <ShellConfirmAction
          :disabled="
            busy ||
            (!entry.interrupted &&
              ['starting', 'running', 'stopping'].includes(entry.snapshot.state))
          "
          label="Delete"
          title="Delete stored run?"
          description="The stored measurements will be removed. Comparisons using this run will become unavailable."
          @confirm="remove(entry.id)"
        />
      </li>
    </ul>

    <UButton
      :disabled="busy || entries.length < 25"
      variant="outline"
      color="neutral"
      label="Older runs"
      @click="refresh(true)"
    />

    <RunComparison
      v-if="candidate"
      :key="candidate.id"
      :candidate="candidate"
      :pinned-baseline="baseline"
      :revision="revision"
      @updated="updated"
    />

    <div
      :class="selected.length > 1 ? 'lg:grid-cols-2' : ''"
      class="grid gap-6"
    >
      <RunHistoryDetail
        v-for="entry in selected"
        :key="entry.id"
        :entry="entry"
      />
    </div>
  </ShellPanel>
</template>
