<script setup lang="ts">
  import type { HistoryEntry } from '~/types/contracts';
  import { invoke } from '@tauri-apps/api/core';

  const entries = shallowRef<HistoryEntry[]>([]);
  const selected = shallowRef<HistoryEntry[]>([]);
  const error = shallowRef<string | null>(null);
  const status = shallowRef<string | null>(null);
  const busy = shallowRef<boolean>(false);
  const cursor = shallowRef<string | null>(null);

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
      cursor.value = page.at(-1)?.id ?? null;
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
  <section
    class="mt-8 space-y-5 rounded-lg border p-6"
    aria-label="Run history"
  >
    <div class="flex items-center justify-between gap-4">
      <h2 class="font-medium">Run history</h2>

      <Button
        :disabled="busy"
        variant="outline"
        @click="refresh()"
      >
        Refresh latest
      </Button>
    </div>

    <p class="text-muted-foreground text-sm">
      Summaries remain until deletion. Select up to two runs to compare observed results. Matching
      origin, method, and workload does not guarantee identical request data or target conditions.
    </p>

    <p
      v-if="error || status"
      class="text-destructive text-sm"
      role="alert"
    >
      {{
        error ||
        `${status}: History may be incomplete. Current execution results remain available above.`
      }}
    </p>

    <p
      v-if="!entries.length"
      class="text-muted-foreground text-sm"
    >
      No stored runs on this page.
    </p>

    <ul class="space-y-3">
      <li
        v-for="entry in entries"
        :key="entry.id"
        class="flex flex-wrap items-center gap-3 border-b pb-3 text-sm"
      >
        <span class="flex-1">
          {{ new Date(Number(entry.created_at) * 1000).toLocaleString() }} · {{ entry.method }}
          {{ entry.snapshot.confirmation.origin }} ·
          {{ entry.interrupted ? 'interrupted' : entry.snapshot.state }}
          {{ entry.incomplete ? ' · incomplete history' : '' }}
        </span>

        <Button
          :disabled="busy"
          :aria-pressed="selected.some((item) => item.id === entry.id)"
          variant="outline"
          @click="select(entry.id)"
        >
          {{ selected.some((item) => item.id === entry.id) ? 'Remove selection' : 'Select' }}
        </Button>

        <Button
          :disabled="
            busy ||
            (!entry.interrupted &&
              ['starting', 'running', 'stopping'].includes(entry.snapshot.state))
          "
          variant="ghost"
          @click="remove(entry.id)"
        >
          Delete
        </Button>
      </li>
    </ul>

    <Button
      :disabled="busy || entries.length < 25"
      variant="outline"
      @click="refresh(true)"
    >
      Older runs
    </Button>

    <div class="grid gap-6 lg:grid-cols-2">
      <RunHistoryDetail
        v-for="entry in selected"
        :key="entry.id"
        :entry="entry"
      />
    </div>
  </section>
</template>
