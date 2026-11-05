<template>
  <Topbar />

  <div class="flex h-[calc(100svh-2.3125rem)] min-h-0">
    <ShellNavigation />

    <ScrollArea class="min-w-0 flex-1">
      <main
        id="main-content"
        class="mx-auto w-full max-w-5xl px-6 py-10 sm:px-10"
        tabindex="-1"
      >
        <NuxtPage />
      </main>
    </ScrollArea>
  </div>
</template>
