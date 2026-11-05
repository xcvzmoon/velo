<template>
  <div>
    <ShellPageHeading
      title="Dashboard"
      description="A local studio for understanding how your APIs perform under load."
    />

    <ShellEmptyState
      title="Your workspace is ready"
      description="Load testing is coming next. For now, you can verify the connection to Velo's native foundation in Settings."
    >
      <Button
        variant="outline"
        as-child
      >
        <NuxtLink to="/settings">Open Settings</NuxtLink>
      </Button>
    </ShellEmptyState>
  </div>
</template>
