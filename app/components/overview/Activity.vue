<script setup lang="ts">
  import type { RunSnapshot } from '~/types/contracts';
  import type { OverviewTimeline } from '~/utils/overview';

  const props = defineProps<{
    snapshot: RunSnapshot | null;
    timeline: OverviewTimeline | null;
    active: boolean;
    busy: boolean;
    connected: boolean;
    gaps: boolean;
    stored: boolean;
    interrupted: boolean;
  }>();
  const emit = defineEmits<{ stop: [] }>();
  const progress = computed<number>(() =>
    props.snapshot
      ? Math.min(
          100,
          Number(props.snapshot.scheduling_us) / (props.snapshot.confirmation.duration_ms * 10),
        )
      : 0,
  );
</script>

<template>
  <UCard
    :ui="{ header: 'bg-elevated/50 rounded-t-xl p-4 sm:px-5', body: 'p-4 sm:p-5' }"
    class="bg-default ring-default min-w-0 rounded-xl shadow-sm"
  >
    <template #header>
      <div class="flex flex-wrap items-center justify-between gap-3">
        <h2 class="text-highlighted text-sm font-semibold">Run activity</h2>

        <UBadge
          v-if="snapshot"
          :label="interrupted ? 'interrupted' : snapshot.state"
          :color="
            interrupted
              ? 'warning'
              : active
                ? 'primary'
                : snapshot.state === 'failed'
                  ? 'error'
                  : 'neutral'
          "
          variant="subtle"
          class="text-highlighted capitalize"
        />
      </div>
    </template>

    <div
      v-if="snapshot"
      class="space-y-5"
    >
      <div class="flex flex-wrap items-start justify-between gap-3">
        <div class="min-w-0">
          <p class="text-highlighted font-medium break-all">{{ snapshot.confirmation.origin }}</p>

          <p class="text-muted mt-1 text-xs">
            {{ stored ? 'Stored run' : `Run ${snapshot.run_id}` }} ·
            {{ snapshot.confirmation.workload.kind }} workload
          </p>
        </div>

        <UButton
          v-if="active"
          :disabled="busy || snapshot.state === 'stopping'"
          :label="snapshot.state === 'stopping' ? 'Draining…' : 'Stop run'"
          color="error"
          variant="soft"
          @click="emit('stop')"
        />

        <UButton
          v-else
          to="/results"
          label="Open results"
          color="neutral"
          variant="ghost"
        />
      </div>

      <dl
        class="bg-elevated/50 ring-default grid grid-cols-2 gap-4 rounded-lg p-4 text-sm ring ring-inset sm:grid-cols-3"
      >
        <div>
          <dt class="text-muted text-xs">Elapsed / target</dt>

          <dd class="text-highlighted mt-1 font-mono tabular-nums">
            {{ (Number(snapshot.actual_us) / 1e6).toFixed(1) }} /
            {{ snapshot.confirmation.duration_ms / 1000 }} s
          </dd>
        </div>

        <div>
          <dt class="text-muted text-xs">Active users</dt>

          <dd class="text-highlighted mt-1 font-mono tabular-nums">
            {{ snapshot.active_vus }}
            <span class="text-muted text-xs">/ {{ snapshot.desired_vus }} desired</span>
          </dd>
        </div>

        <div>
          <dt class="text-muted text-xs">In flight</dt>

          <dd class="text-highlighted mt-1 font-mono tabular-nums">
            {{ snapshot.metrics.counters.in_flight }}
          </dd>
        </div>
      </dl>

      <UProgress
        v-if="active"
        :model-value="progress"
        :max="100"
        :get-value-label="() => 'Scheduling progress'"
        size="xs"
      />

      <div
        v-if="timeline?.seconds.length"
        class="grid min-w-0 gap-3 2xl:grid-cols-2"
      >
        <RunChart
          :seconds="timeline.seconds"
          :values="timeline.requests"
          :warmup-seconds="snapshot.measurement.warmup_ms / 1000"
          :automatic-stop-seconds="
            snapshot.automatic_stop ? Number(snapshot.automatic_stop.at_us) / 1e6 : undefined
          "
          title="Request rate"
          unit="req/s"
        />

        <RunChart
          :seconds="timeline.seconds"
          :values="timeline.latency"
          :warmup-seconds="snapshot.measurement.warmup_ms / 1000"
          :automatic-stop-seconds="
            snapshot.automatic_stop ? Number(snapshot.automatic_stop.at_us) / 1e6 : undefined
          "
          title="Response p95"
          unit="ms"
        />
      </div>

      <UEmpty
        v-else
        :title="stored ? 'Timeline available in Results' : 'Waiting for interval samples'"
        :description="
          stored
            ? 'Open the stored run in Results to inspect its timeline and diagnostics.'
            : 'Charts appear when interval measurements arrive. Summary values remain available above.'
        "
        :ui="{ root: 'min-h-40 py-6', title: 'text-sm', description: 'text-xs' }"
        icon="i-hugeicons-chart-line-data-01"
        variant="subtle"
      />

      <p
        v-if="!stored"
        class="text-muted text-xs leading-relaxed"
      >
        {{
          connected
            ? 'Live updates connected.'
            : 'Recovering native totals; live updates are unavailable.'
        }}
        {{ gaps ? 'Some intervals are missing; charts preserve the gaps.' : '' }}
        Charts include warm-up. Full diagnostics are available in Results.
      </p>
    </div>

    <UEmpty
      v-else
      :ui="{ root: 'min-h-64', title: 'text-base', description: 'text-sm' }"
      title="Your first run starts here"
      description="Configure a request or scenario, review its workload, and authorize the run. Its activity will appear here."
      icon="i-hugeicons-chart-line-data-01"
      variant="naked"
    >
      <template #actions>
        <UButton
          to="/tests"
          label="Configure a test"
          color="neutral"
        />
      </template>
    </UEmpty>
  </UCard>
</template>
