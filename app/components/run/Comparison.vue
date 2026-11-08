<script setup lang="ts">
  import { invoke } from '@tauri-apps/api/core';

  const props = defineProps<{
    candidate: HistoryEntry;
    pinnedBaseline: string | null;
    revision: number;
  }>();
  const emit = defineEmits<{ updated: [entry: HistoryEntry] }>();
  const report = shallowRef<ComparisonReport | null>(null);
  const settings = shallowRef<ComparisonSettings | null>(null);
  const error = shallowRef<string | null>(null);
  const message = shallowRef<string | null>(null);
  const busy = shallowRef<boolean>(false);
  const metadataDirty = shallowRef<boolean>(false);
  const budgetsDirty = shallowRef<boolean>(false);
  const generation = shallowRef<number>(0);

  async function load(): Promise<void> {
    const current = ++generation.value;
    busy.value = true;
    error.value = null;
    budgetsDirty.value = false;
    report.value = null;
    settings.value = null;
    try {
      const saved = await invoke<ComparisonSettings | null>('load_comparison_settings', {
        id: props.candidate.id,
      });
      if (current !== generation.value) {
        return;
      }
      const baseline = saved?.baseline_id ?? props.pinnedBaseline;
      if (!baseline) {
        return;
      }
      const value = saved ?? {
        baseline_id: baseline,
        budgets: [],
        configuration_reviewed: false,
      };
      const result = await invoke<ComparisonReport>('save_comparison', {
        id: props.candidate.id,
        settings: value,
      });
      if (current !== generation.value) {
        return;
      }
      settings.value = value;
      report.value = result;
    } catch {
      if (current === generation.value) {
        error.value = 'COMPARISON_UNAVAILABLE: The stored comparison could not be loaded.';
      }
    } finally {
      if (current === generation.value) {
        busy.value = false;
      }
    }
  }

  async function save(value: ComparisonSettings): Promise<void> {
    busy.value = true;
    error.value = null;
    message.value = null;
    try {
      report.value = await invoke<ComparisonReport>('save_comparison', {
        id: props.candidate.id,
        settings: value,
      });
      settings.value = value;
      budgetsDirty.value = false;
    } catch {
      error.value = 'COMPARISON_SAVE_FAILED: The comparison could not be saved.';
    } finally {
      busy.value = false;
    }
  }

  async function usePinned(): Promise<void> {
    if (!props.pinnedBaseline) {
      return;
    }
    await save({
      baseline_id: props.pinnedBaseline,
      budgets: settings.value?.budgets ?? [],
      configuration_reviewed: false,
    });
  }

  async function metadata(value: RunMetadata): Promise<void> {
    busy.value = true;
    error.value = null;
    try {
      await invoke('update_run_metadata', {
        id: props.candidate.id,
        metadata: value,
      });
      const entry = await invoke<HistoryEntry>('load_history', { id: props.candidate.id });
      if (report.value) {
        report.value = {
          ...report.value,
          candidate: entry,
        };
      }
      emit('updated', entry);
      metadataDirty.value = false;
      message.value = 'Release context saved.';
    } catch {
      error.value = 'METADATA_SAVE_FAILED: Release context could not be saved.';
    } finally {
      busy.value = false;
    }
  }

  async function exportReport(format: 'html' | 'json' | 'junit'): Promise<void> {
    busy.value = true;
    error.value = null;
    try {
      const saved = await invoke<boolean>('export_comparison', {
        id: props.candidate.id,
        format,
      });
      message.value = saved ? 'Report saved.' : 'Export cancelled.';
    } catch {
      error.value = 'REPORT_EXPORT_FAILED: The report could not be saved.';
    } finally {
      busy.value = false;
    }
  }

  watch(
    () => [props.candidate.id, props.revision] as const,
    () => void load(),
    { immediate: true },
  );
  onBeforeUnmount(() => {
    generation.value++;
  });
</script>

<template>
  <ShellPanel
    variant="inset"
    aria-label="Baseline comparison"
  >
    <template #header>
      <h3 class="font-medium">Candidate release and regression report</h3>
    </template>

    <p class="text-muted text-sm">Candidate {{ candidate.id }}</p>

    <p
      v-if="error"
      class="text-error text-sm"
      role="alert"
    >
      {{ error }}
    </p>

    <p
      v-if="message"
      class="text-muted text-sm"
      role="status"
    >
      {{ message }}
    </p>

    <RunMetadataForm
      :key="candidate.id"
      :metadata="candidate.metadata"
      :disabled="busy"
      @save="metadata"
      @dirty="metadataDirty = true"
    />

    <UButton
      :disabled="busy || !pinnedBaseline || pinnedBaseline === settings?.baseline_id"
      variant="outline"
      color="neutral"
      label="Use pinned baseline"
      @click="usePinned"
    />

    <p
      v-if="!settings && !busy"
      class="text-muted text-sm"
    >
      Pin a historical run to compare this candidate.
    </p>

    <template v-if="report && settings">
      <RunComparisonEvidence :report="report" />

      <AiAssistant
        :evidence="selectAiEvidence(candidate, report)"
        :disabled="busy || metadataDirty || budgetsDirty"
        task="explain"
      />

      <RunRegressionSettingsForm
        :key="`${candidate.id}-${settings.baseline_id}`"
        :settings="settings"
        :disabled="busy"
        @save="save"
        @dirty="budgetsDirty = true"
      />

      <ul
        class="space-y-2 text-sm"
        aria-label="Regression budgets"
      >
        <li
          v-for="(result, index) in report.regressions"
          :key="index"
        >
          {{ result.budget.metric }} · {{ result.verdict }} · worsening
          {{ result.worsening ?? 'unavailable' }} / limit {{ result.budget.limit }}
          {{ result.budget.percentage ? '%' : 'metric units' }}
        </li>
      </ul>

      <p
        v-if="metadataDirty || budgetsDirty"
        class="text-muted text-sm"
      >
        Save release context and budget edits before exporting.
      </p>

      <div class="flex flex-wrap gap-2">
        <UButton
          :disabled="busy || metadataDirty || budgetsDirty"
          variant="outline"
          color="neutral"
          label="Export readable report"
          @click="exportReport('html')"
        />

        <UButton
          :disabled="busy || metadataDirty || budgetsDirty"
          variant="outline"
          color="neutral"
          label="Export JSON v1"
          @click="exportReport('json')"
        />

        <UButton
          :disabled="busy || metadataDirty || budgetsDirty"
          variant="outline"
          color="neutral"
          label="Export JUnit"
          @click="exportReport('junit')"
        />
      </div>
    </template>
  </ShellPanel>
</template>
