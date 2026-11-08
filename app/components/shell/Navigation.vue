<script setup lang="ts">
  import type { NavigationMenuItem } from '@nuxt/ui';

  const links: NavigationMenuItem[][] = [
    [
      {
        label: 'Overview',
        to: '/',
        exact: true,
        icon: 'i-hugeicons-dashboard-square-01',
      },
      {
        label: 'API workspace',
        to: '/api',
        icon: 'i-hugeicons-api',
      },
      {
        label: 'Test studio',
        to: '/tests',
        icon: 'i-hugeicons-test-tube-01',
      },
      {
        label: 'Results',
        to: '/results',
        icon: 'i-hugeicons-chart-line-data-01',
      },
    ],
    [
      {
        label: 'Settings',
        to: '/settings',
        icon: 'i-hugeicons-settings-02',
      },
    ],
  ];
</script>

<template>
  <aside class="bg-muted/30 flex w-48 shrink-0 flex-col border-r p-3 lg:w-56">
    <a
      href="#main-content"
      class="bg-default text-primary sr-only rounded-md focus:not-sr-only focus:p-2"
    >
      Skip to content
    </a>

    <div class="flex items-center gap-3 px-2 py-5">
      <div
        class="bg-primary/10 text-primary flex size-8 items-center justify-center rounded-lg font-mono text-lg font-semibold"
      >
        v.
      </div>

      <div>
        <p class="text-highlighted text-sm font-semibold tracking-tight">Velo</p>
        <p class="text-muted text-[11px]">API performance studio</p>
      </div>
    </div>

    <p class="text-dimmed px-2 pt-3 pb-2 text-[10px] font-medium tracking-widest uppercase">
      Workspace
    </p>

    <UNavigationMenu
      :items="links"
      orientation="vertical"
      aria-label="Main navigation"
    />

    <div class="mt-auto border-t px-2 pt-4 pb-2">
      <UBadge
        color="neutral"
        variant="subtle"
        size="sm"
        label="Local workspace"
      />

      <p class="text-muted mt-2 text-[11px] leading-relaxed">
        Tests, secrets, and results stay on your machine. AI sharing requires your review.
      </p>
    </div>
  </aside>
</template>
