<script setup lang="ts">
  const links = [
    {
      label: 'Dashboard',
      to: '/',
      number: '01',
    },
    {
      label: 'API workspace',
      to: '/api',
      number: '02',
    },
    {
      label: 'Tests',
      to: '/tests',
      number: '03',
    },
    {
      label: 'Results',
      to: '/results',
      number: '04',
    },
    {
      label: 'Settings',
      to: '/settings',
      number: '05',
    },
  ];
</script>

<template>
  <aside class="bg-sidebar/30 flex w-40 shrink-0 flex-col border-r px-3 py-6 sm:w-48">
    <a
      href="#main-content"
      class="sr-only focus:not-sr-only focus:p-2"
    >
      Skip to content
    </a>

    <p class="text-muted-foreground px-3 pb-5 text-[10px] font-medium tracking-[0.18em] uppercase">
      Workspace
    </p>

    <nav
      aria-label="Main navigation"
      class="space-y-1"
    >
      <NuxtLink
        v-for="{ label, to, number } in links"
        :key="to"
        :to="to"
        class="text-muted-foreground hover:bg-accent hover:text-foreground focus-visible:outline-ring flex items-center gap-3 rounded-md px-3 py-2.5 text-sm focus-visible:outline-2"
        exact-active-class="bg-accent text-foreground"
      >
        <span
          aria-hidden="true"
          class="font-mono text-[10px] opacity-50"
        >
          {{ number }}
        </span>
        {{ label }}
      </NuxtLink>
    </nav>

    <div class="mt-auto px-3 pt-8">
      <p class="text-xs font-medium">Local workspace</p>

      <p class="text-muted-foreground mt-1 text-[11px] leading-relaxed">
        Your machine.
        <br />
        Your infrastructure.
      </p>
    </div>
  </aside>
</template>
