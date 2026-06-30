<script setup lang="ts">
import { inject } from 'vue'
import { X } from 'lucide-vue-next'

const { open, close } = inject<any>('sheetContext')

interface Props {
  side?: 'left' | 'right' | 'top' | 'bottom'
}

withDefaults(defineProps<Props>(), { side: 'right' })
</script>

<template>
  <Transition name="sheet">
    <div v-if="open" class="fixed inset-0 z-50 sheet-wrapper">
      <div class="fixed inset-0 bg-background/60 backdrop-blur-sm" @click="close" />
      <div class="fixed bottom-0 right-0 top-0 z-50 flex w-full max-w-sm flex-col border-l bg-card shadow-lg sheet-panel">
        <div class="flex items-center justify-between border-b px-4 py-3">
          <div class="text-sm font-medium">
            <slot name="title" />
          </div>
          <button
            class="flex h-6 w-6 items-center justify-center rounded text-muted-foreground hover:text-foreground transition-colors"
            @click="close"
          >
            <X class="h-4 w-4" />
          </button>
        </div>
        <div v-if="$slots.description" class="border-b px-4 py-2 text-[11px] text-muted-foreground">
          <slot name="description" />
        </div>
        <div class="flex-1 overflow-y-auto p-4">
          <slot />
        </div>
      </div>
    </div>
  </Transition>
</template>

<style scoped>
.sheet-enter-active {
  transition: opacity 0.2s ease;
}
.sheet-leave-active {
  transition: opacity 0.2s ease;
}
.sheet-enter-from,
.sheet-leave-to {
  opacity: 0;
}

.sheet-panel {
  transition: transform 0.3s ease;
}
.sheet-enter-from .sheet-panel,
.sheet-leave-to .sheet-panel {
  transform: translateX(100%);
}
</style>
