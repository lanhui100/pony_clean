<script setup lang="ts">
import { type HTMLAttributes } from 'vue'
import { cn } from '../../../lib/utils'

interface Props {
  checked?: boolean
  disabled?: boolean
  class?: HTMLAttributes['class']
}

const props = withDefaults(defineProps<Props>(), {
  checked: false,
  disabled: false,
})

const emit = defineEmits<{ 'update:checked': [value: boolean] }>()
</script>

<template>
  <button
    type="button"
    role="switch"
    :aria-checked="checked"
    :aria-disabled="disabled"
    :disabled="disabled"
    :class="cn(
      'relative h-4 w-7 shrink-0 rounded-full transition-colors',
      checked ? 'bg-success/80' : 'bg-white/15',
      props.class,
    )"
    @click="emit('update:checked', !checked)"
  >
    <span
      class="absolute top-0.5 h-3 w-3 rounded-full bg-white shadow transition-all"
      :class="checked ? 'left-[14px]' : 'left-0.5'"
    />
  </button>
</template>
