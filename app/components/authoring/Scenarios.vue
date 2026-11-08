<script setup lang="ts">
  import type { RunConfig, Scenario, ScenarioPlan, ScenarioStep } from '~/types/contracts';

  type Selection = { phase: 'setup' | 'cleanup' } | { phase: 'load'; index: number };
  type Settings = { seed: number; id: string; weight: number };
  const props = defineProps<{ plan: ScenarioPlan | null; base: RunConfig; disabled: boolean }>();
  const emit = defineEmits<{ change: [plan: ScenarioPlan | null]; dirty: [value: boolean] }>();
  const selection = shallowRef<Selection>({
    phase: 'load',
    index: 0,
  });
  const stepIndex = shallowRef<number>(0);
  const revision = shallowRef<number>(0);
  const settingsDirty = shallowRef<boolean>(false);
  const stepDirty = shallowRef<boolean>(false);
  const requestValid = shallowRef<boolean>(true);
  const error = shallowRef<string | null>(null);
  const selectedScenario = computed<Scenario | null>(() =>
    selection.value.phase === 'load'
      ? (props.plan?.scenarios[selection.value.index] ?? null)
      : null,
  );
  const steps = computed<ScenarioStep[]>(() => {
    if (!props.plan) {
      return [];
    }
    return selection.value.phase === 'load'
      ? (selectedScenario.value?.steps ?? [])
      : props.plan[selection.value.phase];
  });
  const selectedStep = computed<ScenarioStep | null>(() => steps.value[stepIndex.value] ?? null);
  const selectionKey = computed<string>(
    () =>
      `${selection.value.phase}-${selection.value.phase === 'load' ? selection.value.index : ''}-${revision.value}`,
  );
  const reservedScenarioIds = computed<string[]>(() => {
    const ids: string[] = [];
    for (const [index, { id }] of (props.plan?.scenarios ?? []).entries()) {
      if (selection.value.phase !== 'load' || selection.value.index !== index) {
        ids.push(id);
      }
    }
    return ids;
  });
  const reservedStepIds = computed<string[]>(() => {
    const ids: string[] = [];
    for (const [index, { id }] of steps.value.entries()) {
      if (index !== stepIndex.value) {
        ids.push(id);
      }
    }
    return ids;
  });
  const totalSteps = computed<number>(() => {
    if (!props.plan) {
      return 0;
    }
    let count = props.plan.setup.length + props.plan.cleanup.length;
    for (const { steps: scenarioSteps } of props.plan.scenarios) {
      count += scenarioSteps.length;
    }
    return count;
  });
  const dirty = computed<boolean>(
    () => settingsDirty.value || stepDirty.value || !requestValid.value || error.value !== null,
  );

  function newStep(id: string): ScenarioStep {
    const { url, method, headers, query, body, auth, assertions } = props.base;
    return {
      id,

      think_ms: 0,

      on_failure: 'stop',

      extractions: [],

      request: {
        url,

        method,

        headers: headers.map(({ name, value }) => ({ name, value })),

        query: query.map(({ name, value }) => ({ name, value })),

        body: structuredClone(body),

        auth: structuredClone(auth),

        assertions: structuredClone(assertions),
      },
    };
  }
  function enable(): void {
    emit('change', {
      seed: 0,

      setup: [],

      cleanup: [],

      scenarios: [
        {
          id: 'journey',
          weight: 1,
          steps: [newStep('step-1')],
        },
      ],
    });
  }
  function select(value: Selection): void {
    selection.value = value;
    stepIndex.value = 0;
    discard();
  }
  function discard(): void {
    settingsDirty.value = false;
    stepDirty.value = false;
    requestValid.value = true;
    error.value = null;
    revision.value++;
  }
  function applySettings(value: Settings): void {
    if (!props.plan) {
      return;
    }
    const plan = props.plan;
    if (selection.value.phase === 'load') {
      const index = selection.value.index;
      if (plan.scenarios.some(({ id }, candidate) => candidate !== index && id === value.id)) {
        error.value = 'Scenario names must be unique.';
        settingsDirty.value = true;
        return;
      }
      emit('change', {
        ...plan,
        seed: value.seed,

        scenarios: plan.scenarios.map((scenario, candidate) =>
          candidate === index
            ? {
                ...scenario,
                id: value.id,
                weight: value.weight,
              }
            : scenario,
        ),
      });
    } else {
      emit('change', {
        ...plan,
        seed: value.seed,
      });
    }
    error.value = null;
  }
  function replaceSteps(value: ScenarioStep[]): void {
    if (!props.plan) {
      return;
    }
    if (selection.value.phase === 'load') {
      const index = selection.value.index;
      emit('change', {
        ...props.plan,
        scenarios: props.plan.scenarios.map((scenario, candidate) =>
          candidate === index
            ? {
                ...scenario,
                steps: value,
              }
            : scenario,
        ),
      });
    } else {
      emit('change', {
        ...props.plan,
        [selection.value.phase]: value,
      });
    }
  }
  function applyStep(value: ScenarioStep): void {
    if (steps.value.some(({ id }, index) => index !== stepIndex.value && id === value.id)) {
      error.value = 'Step names must be unique within their scenario or lifecycle phase.';
      stepDirty.value = true;
      return;
    }
    replaceSteps(steps.value.map((step, index) => (index === stepIndex.value ? value : step)));
    error.value = null;
  }
  function addScenario(): void {
    if (!props.plan) {
      return;
    }
    let index = 1;
    while (props.plan.scenarios.some(({ id }) => id === `journey-${index}`)) {
      index++;
    }
    const next = props.plan.scenarios.length;
    emit('change', {
      ...props.plan,
      scenarios: [
        ...props.plan.scenarios,
        {
          id: `journey-${index}`,
          weight: 1,
          steps: [newStep('step-1')],
        },
      ],
    });
    select({
      phase: 'load',
      index: next,
    });
  }
  function removeScenario(): void {
    if (!props.plan || selection.value.phase !== 'load') {
      return;
    }
    const index = selection.value.index;
    emit('change', {
      ...props.plan,
      scenarios: props.plan.scenarios.filter((_, candidate) => candidate !== index),
    });
    select({
      phase: 'load',
      index: 0,
    });
  }
  function addStep(): void {
    let index = 1;
    while (steps.value.some(({ id }) => id === `step-${index}`)) {
      index++;
    }
    const next = steps.value.length;
    replaceSteps([...steps.value, newStep(`step-${index}`)]);
    stepIndex.value = next;
    discard();
  }
  function removeStep(): void {
    replaceSteps(steps.value.filter((_, index) => index !== stepIndex.value));
    stepIndex.value = Math.max(0, stepIndex.value - 1);
    discard();
  }
  function moveStep(offset: number): void {
    const next = stepIndex.value + offset;
    const reordered = [...steps.value];
    const current = reordered[stepIndex.value];
    const target = reordered[next];
    if (!current || !target) {
      return;
    }
    reordered[next] = current;
    reordered[stepIndex.value] = target;
    replaceSteps(reordered);
    stepIndex.value = next;
    discard();
  }
  watch(dirty, (value) => emit('dirty', value));
</script>

<template>
  <ShellPanel
    variant="section"
    aria-label="Scenario authoring"
  >
    <template #header>
      <h2 class="text-highlighted font-semibold">Scenarios and user sessions</h2>
    </template>

    <p class="text-muted text-sm">
      Each admitted iteration selects one weighted scenario and executes its steps in order. VU
      workloads repeat iterations; arrival workloads admit iterations. Requests per second and
      iterations per second are separate measurements. Weights are relative probabilities, so small
      runs may differ substantially from their proportions. The seed reproduces selection in global
      admission order; scheduling determines how many iterations are admitted.
    </p>

    <UButton
      v-if="!plan"
      :disabled="disabled"
      variant="outline"
      color="neutral"
      label="Enable multi-step scenarios"
      @click="enable"
    />

    <template v-else>
      <p class="text-muted text-sm">
        Shared variables use
        <code v-text="'{{name}}'" />
        ; the active environment overrides shared values and dataset fields override both. Extracted
        values use the separate
        <code v-text="'{{session:name}}'" />
        namespace. Each VU keeps its own cookies and extracted values across iterations. Arrival
        iterations start with isolated sessions.
      </p>

      <p class="text-muted text-sm">
        Setup runs once before measured traffic. Its values seed isolated user sessions. Cleanup
        runs once after measured traffic using setup state, including after stopping or failure
        where possible. Setup and cleanup each have a 30-second deadline and use the first prepared
        dataset record when data is configured. Put per-user record removal in a journey step. All
        requests must use the reviewed origin; session values can change URL paths and queries only.
      </p>

      <p class="text-muted text-xs">
        A plan supports 16 scenarios and 64 total steps, with at most 32 steps per scenario. Each
        user has a 32 KiB state budget for up to 32 extracted values and 32 cookies. Each extracted
        value is limited to 4 KiB. JSON extraction uses the configured response inspection limit.
      </p>

      <div
        class="flex flex-wrap gap-2"
        aria-label="Scenario and lifecycle selection"
      >
        <UButton
          :disabled="disabled || dirty"
          :variant="selection.phase === 'setup' ? 'solid' : 'outline'"
          label="Setup"
          @click="select({ phase: 'setup' })"
        />

        <UButton
          v-for="(scenario, index) in plan.scenarios"
          :key="index"
          :disabled="disabled || dirty"
          :variant="selection.phase === 'load' && selection.index === index ? 'solid' : 'outline'"
          :label="`${scenario.id} · weight ${scenario.weight}`"
          @click="select({ phase: 'load', index })"
        />

        <UButton
          :disabled="disabled || dirty"
          :variant="selection.phase === 'cleanup' ? 'solid' : 'outline'"
          label="Cleanup"
          @click="select({ phase: 'cleanup' })"
        />
      </div>

      <div class="flex flex-wrap gap-2">
        <UButton
          :disabled="disabled || dirty || plan.scenarios.length >= 16 || totalSteps >= 64"
          variant="outline"
          color="neutral"
          label="Add scenario"
          @click="addScenario"
        />

        <UButton
          v-if="selection.phase === 'load'"
          :disabled="disabled || dirty || plan.scenarios.length <= 1"
          variant="outline"
          color="neutral"
          label="Remove scenario"
          @click="removeScenario"
        />

        <UButton
          :disabled="disabled || dirty"
          variant="outline"
          color="neutral"
          label="Use standalone request"
          @click="emit('change', null)"
        />

        <UButton
          v-if="dirty"
          :disabled="disabled"
          variant="outline"
          color="neutral"
          label="Discard unapplied settings"
          @click="discard"
        />
      </div>

      <p
        v-if="dirty"
        class="text-muted text-sm"
        role="status"
      >
        Apply the edited settings and correct request errors before switching steps or reviewing
        load.
      </p>

      <p
        v-if="error"
        class="text-error text-sm"
        role="alert"
      >
        {{ error }}
      </p>

      <AuthoringScenarioSettingsForm
        :key="selectionKey"
        :seed="plan.seed"
        :scenario="selectedScenario"
        :reserved-ids="reservedScenarioIds"
        :disabled="disabled"
        @apply="applySettings"
        @dirty="settingsDirty = $event"
      />

      <div
        class="flex flex-wrap gap-2"
        aria-label="Ordered steps"
      >
        <UButton
          v-for="(step, index) in steps"
          :key="index"
          :disabled="disabled || dirty"
          :variant="stepIndex === index ? 'solid' : 'outline'"
          :label="`${index + 1}. ${step.id}`"
          @click="
            stepIndex = index;
            discard();
          "
        />

        <UButton
          :disabled="disabled || dirty || steps.length >= 32 || totalSteps >= 64"
          variant="outline"
          color="neutral"
          label="Add step"
          @click="addStep"
        />
      </div>

      <template v-if="selectedStep">
        <div class="flex flex-wrap gap-2">
          <UButton
            :disabled="disabled || dirty || stepIndex === 0"
            variant="outline"
            color="neutral"
            label="Move earlier"
            @click="moveStep(-1)"
          />

          <UButton
            :disabled="disabled || dirty || stepIndex === steps.length - 1"
            variant="outline"
            color="neutral"
            label="Move later"
            @click="moveStep(1)"
          />

          <UButton
            :disabled="disabled || dirty || (selection.phase === 'load' && steps.length <= 1)"
            variant="outline"
            color="neutral"
            label="Remove step"
            @click="removeStep"
          />
        </div>

        <AuthoringScenarioStepForm
          :key="`${selectionKey}-${stepIndex}`"
          :step="selectedStep"
          :reserved-ids="reservedStepIds"
          :base="base"
          :disabled="disabled"
          @change="applyStep"
          @dirty="stepDirty = $event"
          @valid="requestValid = $event"
        />
      </template>

      <p
        v-else
        class="text-muted text-sm"
      >
        No {{ selection.phase }} steps configured.
      </p>
    </template>
  </ShellPanel>
</template>
