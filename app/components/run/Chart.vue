<script setup lang="ts">
  import uPlot from 'uplot';

  const props = defineProps<{
    title: string;
    unit: string;
    values: (number | null)[];
    seconds: number[];
    comparisonValues?: (number | null)[];
  }>();

  const host = useTemplateRef<HTMLDivElement>('host');

  let chart: uPlot | null = null;
  let observer: ResizeObserver | null = null;

  function data(): uPlot.AlignedData {
    return props.comparisonValues
      ? [props.seconds, props.values, props.comparisonValues]
      : [props.seconds, props.values];
  }

  onMounted(() => {
    const element = host.value;
    if (!element) {
      return;
    }

    chart = new uPlot(
      {
        width: Math.max(element.clientWidth, 200),
        height: 190,
        scales: {
          x: { time: false },
          y: { range: (_chart, min, max) => [Math.min(0, min), Math.max(1, max * 1.1)] },
        },
        series: [
          { label: 'Elapsed (s)' },
          {
            label: props.comparisonValues ? 'Actual VUs' : props.unit,
            stroke: '#38bdf8',
            width: 1.5,
            spanGaps: false,
          },
          ...(props.comparisonValues
            ? [
                {
                  label: 'Desired VUs',
                  stroke: '#fbbf24',
                  width: 1.5,
                  dash: [5, 4],
                  spanGaps: false,
                },
              ]
            : []),
        ],
        axes: [
          {
            stroke: '#94a3b8',
            grid: { stroke: '#64748b20' },
            ticks: { stroke: '#64748b30' },
            font: '11px monospace',
          },
          {
            stroke: '#94a3b8',
            grid: { stroke: '#64748b20' },
            ticks: { stroke: '#64748b30' },
            font: '11px monospace',
            size: 54,
          },
        ],
        legend: { show: Boolean(props.comparisonValues) },
      },
      data(),
      element,
    );

    observer = new ResizeObserver(([entry]) => {
      if (entry) {
        chart?.setSize({
          width: Math.max(entry.contentRect.width, 200),
          height: 190,
        });
      }
    });
    observer.observe(element);
  });

  watch(
    () => [props.values, props.comparisonValues],
    () => chart?.setData(data()),
  );

  onBeforeUnmount(() => {
    observer?.disconnect();
    chart?.destroy();
    chart = null;
  });
</script>

<template>
  <section class="min-w-0 rounded-lg border p-4">
    <div class="mb-4 flex items-baseline justify-between gap-3">
      <h3 class="text-sm font-medium">{{ title }}</h3>
      <span class="text-muted-foreground font-mono text-xs">{{ unit }}</span>
    </div>

    <div
      ref="host"
      :aria-label="`${title}, ${values.length} samples. Current value ${values.at(-1)?.toFixed(2) ?? 'unavailable'} ${unit}.${comparisonValues ? ` Desired ${comparisonValues.at(-1) ?? 'unavailable'} VUs.` : ''}`"
      class="min-h-47.5 w-full overflow-hidden"
      role="img"
    />
  </section>
</template>
