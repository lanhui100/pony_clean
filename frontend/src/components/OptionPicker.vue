<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { ChevronDown } from 'lucide-vue-next'

export interface PickerOption {
  value: string | number
  label: string
}

const props = defineProps<{
  options: PickerOption[]
  modelValue: string | number
  disabled?: boolean
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: string | number): void
}>()

const open = ref(false)

function toggle() {
  if (!props.disabled) open.value = !open.value
}

function select(o: PickerOption) {
  emit('update:modelValue', o.value)
  open.value = false
}

function onDocClick(e: MouseEvent) {
  if (!(e.target as HTMLElement).closest('.option-picker')) {
    open.value = false
  }
}

onMounted(() => document.addEventListener('click', onDocClick))
onUnmounted(() => document.removeEventListener('click', onDocClick))
</script>

<template>
  <div class="option-picker relative flex-1">
    <button
      type="button"
      :disabled="disabled"
      class="flex w-full items-center justify-between gap-1 rounded border border-white/10 bg-white/[0.03] px-2 py-1 text-[11px] text-foreground transition-colors hover:border-primary/40 focus:border-primary/50 focus:outline-none disabled:opacity-50"
      @click.stop="toggle"
    >
      <span class="truncate">{{ options.find(o => o.value === modelValue)?.label ?? modelValue }}</span>
      <ChevronDown
        class="h-3 w-3 shrink-0 text-muted-foreground transition-transform duration-150"
        :class="open ? 'rotate-180' : ''"
      />
    </button>
    <Transition name="picker">
      <div
        v-if="open"
        class="absolute left-0 right-0 z-30 mt-1 overflow-hidden rounded-md border border-white/10 bg-[hsl(30_10%_14%)] shadow-xl shadow-black/40"
      >
        <button
          v-for="o in options"
          :key="String(o.value)"
          type="button"
          class="block w-full px-2 py-1.5 text-left text-[11px] transition-colors hover:bg-primary/15"
          :class="o.value === modelValue ? 'text-primary' : 'text-foreground/90'"
          @click.stop="select(o)"
        >
          {{ o.label }}
        </button>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.picker-enter-active,
.picker-leave-active {
  transition: opacity 0.12s ease, transform 0.12s ease;
}
.picker-enter-from,
.picker-leave-to {
  opacity: 0;
  transform: translateY(-3px);
}
</style>