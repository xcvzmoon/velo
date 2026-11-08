<script setup lang="ts">
  import uPlot from 'uplot';

  const props = defineProps<{
    title: string;
    unit: string;
    percentage?: boolean;
    binary?: boolean;
    values: (number | null)[];
    seconds: number[];
    warmupSeconds?: number;
    automaticStopSeconds?: number;
    comparisonValues?: (number | null)[];
  }>();

  const colorMode = useColorMode();
  const host = useTemplateRef<HTMLDivElement>('host');

  let chart: uPlot | null = null;
  let observer: ResizeObserver | null = null;

  function data(): uPlot.AlignedData {
    return props.comparisonValues
      ? [props.seconds, props.values, props.comparisonValues]
      : [props.seconds, props.values];
  }

  function drawBoundaries(plot: uPlot): void {
    const context = plot.ctx;
    context.save();
    context.textAlign = 'left';
    context.textBaseline = 'alphabetic';
    for (const boundary of [
      {
        seconds: props.warmupSeconds,
        label: 'Measurement starts',
        color: colorMode.value === 'dark' ? '#fbbf24' : '#a16207',
        labelOffset: -17,
      },
      {
        seconds: props.automaticStopSeconds,
        label: 'Automatic stop',
        color: colorMode.value === 'dark' ? '#f87171' : '#b91c1c',
        labelOffset: -3,
      },
    ]) {
      if (
        boundary.seconds === undefined ||
        boundary.seconds <= 0 ||
        boundary.seconds < (plot.scales.x?.min ?? 0) ||
        boundary.seconds > (plot.scales.x?.max ?? 0)
      ) {
        continue;
      }
      const position = plot.valToPos(boundary.seconds, 'x', true);
      context.strokeStyle = boundary.color;
      context.fillStyle = boundary.color;
      context.lineWidth = uPlot.pxRatio;
      context.setLineDash([4 * uPlot.pxRatio, 3 * uPlot.pxRatio]);
      context.beginPath();
      context.moveTo(position, plot.bbox.top);
      context.lineTo(position, plot.bbox.top + plot.bbox.height);
      context.stroke();
      context.font = `${10 * uPlot.pxRatio}px monospace`;
      const width = context.measureText(boundary.label).width;
      context.fillText(
        boundary.label,
        Math.max(
          plot.bbox.left,
          Math.min(position + 4 * uPlot.pxRatio, plot.bbox.left + plot.bbox.width - width),
        ),
        plot.bbox.top + boundary.labelOffset * uPlot.pxRatio,
      );
    }
    context.restore();
  }

  function createChart(): void {
    const element = host.value;
    if (!element) {
      return;
    }

    chart?.destroy();
    chart = new uPlot(
      {
        hooks: { draw: [drawBoundaries] },
        width: Math.max(element.clientWidth, 100),
        height: 190,
        padding: [34, null, null, null],
        scales: {
          x: { time: false },
          y: {
            range: (_chart, min, max) =>
              props.binary
                ? [0, 1.1]
                : props.percentage
                  ? [0, 100]
                  : [Math.min(0, min), Math.max(1, max * 1.1)],
          },
        },
        series: [
          { label: 'Elapsed (s)' },
          {
            label: props.comparisonValues ? 'Actual VUs' : props.unit,
            paths: props.binary ? uPlot.paths.stepped?.({ align: 1 }) : undefined,
            stroke: colorMode.value === 'dark' ? '#2dd4bf' : '#0f766e',
            width: 1.5,
            spanGaps: false,
            points: {
              show: (_plot, seriesIndex, first, last) =>
                hasIsolatedChartPoints(
                  seriesIndex === 1 ? props.values : (props.comparisonValues ?? []),
                  first,
                  last,
                ),
            },
          },
          ...(props.comparisonValues
            ? [
                {
                  label: 'Desired VUs',
                  stroke: colorMode.value === 'dark' ? '#fbbf24' : '#a16207',
                  width: 1.5,
                  dash: [5, 4],
                  spanGaps: false,
                  points: {
                    show: (_plot: uPlot, _seriesIndex: number, first: number, last: number) =>
                      hasIsolatedChartPoints(props.comparisonValues ?? [], first, last),
                  },
                },
              ]
            : []),
        ],
        axes: [
          {
            stroke: colorMode.value === 'dark' ? '#a1a1aa' : '#52525b',
            grid: { stroke: colorMode.value === 'dark' ? '#27272a' : '#e4e4e7' },
            ticks: { stroke: colorMode.value === 'dark' ? '#3f3f46' : '#d4d4d8' },
            font: '11px monospace',
          },
          {
            stroke: colorMode.value === 'dark' ? '#a1a1aa' : '#52525b',
            grid: { stroke: colorMode.value === 'dark' ? '#27272a' : '#e4e4e7' },
            ticks: { stroke: colorMode.value === 'dark' ? '#3f3f46' : '#d4d4d8' },
            font: '11px monospace',
            size: 54,
            splits: props.binary ? () => [0, 1] : undefined,
          },
        ],
        legend: { show: Boolean(props.comparisonValues) },
      },
      data(),
      element,
    );
  }

  watch(() => colorMode.value, createChart);

  onMounted(() => {
    createChart();
    const element = host.value;
    if (!element) {
      return;
    }
    observer = new ResizeObserver(([entry]) => {
      if (entry) {
        chart?.setSize({
          width: Math.max(entry.contentRect.width, 100),
          height: 190,
        });
      }
    });
    observer.observe(element);
  });

  watch(
    () => [
      props.seconds,
      props.values,
      props.comparisonValues,
      props.warmupSeconds,
      props.automaticStopSeconds,
    ],
    () => chart?.setData(data()),
  );

  onBeforeUnmount(() => {
    observer?.disconnect();
    chart?.destroy();
    chart = null;
  });
</script>

<template>
  <section class="border-default min-w-0 rounded-lg border p-4">
    <div class="mb-4 flex items-baseline justify-between gap-3">
      <h3 class="text-sm font-medium">{{ title }}</h3>
      <span class="text-muted font-mono text-xs">{{ unit }}</span>
    </div>

    <p
      v-if="warmupSeconds || automaticStopSeconds !== undefined"
      class="text-muted mb-3 text-xs"
    >
      <span v-if="warmupSeconds">Measurement starts at {{ warmupSeconds }} s.</span>

      <span v-if="automaticStopSeconds !== undefined">
        Automatic stop at {{ automaticStopSeconds.toFixed(3) }} s.
      </span>
    </p>

    <div
      ref="host"
      :aria-label="`${title}, ${values.length} samples. Current value ${values.at(-1)?.toFixed(2) ?? 'unavailable'} ${unit}.${comparisonValues ? ` Desired ${comparisonValues.at(-1) ?? 'unavailable'} VUs.` : ''}`"
      class="min-h-47.5 w-full overflow-hidden"
      role="img"
    />
  </section>
</template>
