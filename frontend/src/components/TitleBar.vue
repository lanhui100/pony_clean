<script setup lang="ts">
import { getCurrentWindow } from '@tauri-apps/api/window'

const appWindow = getCurrentWindow()

async function handleClose() {
  await appWindow.close()
}
</script>

<template>
  <header
    data-tauri-drag-region
    class="flex h-10 items-center justify-between border-b border-border bg-background/80 px-4 backdrop-blur-sm select-none"
  >
    <div class="flex items-center gap-2">
      <span class="text-sm font-semibold text-primary">PonyClean</span>
    </div>
    <button
      class="flex h-6 w-6 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-destructive hover:text-destructive-foreground"
      title="关闭"
      @click="handleClose"
    >
      <svg
        xmlns="http://www.w3.org/2000/svg"
        width="14"
        height="14"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
      >
        <line x1="18" y1="6" x2="6" y2="18" />
        <line x1="6" y1="6" x2="18" y2="18" />
      </svg>
    </button>
  </header>
</template>
