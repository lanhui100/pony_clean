<script setup lang="ts">
import { inject } from 'vue'
import { cn } from '../../../lib/utils'

interface Props {
  class?: string
}

const props = defineProps<Props>()
const { toggle } = inject<{ toggle: () => void; isOpen: import('vue').Ref<boolean> }>('collapsible')!
</script>

<template>
  <button
    :class="cn('flex w-full items-center gap-2', props.class)"
    @click="toggle"
  >
    <slot />
  </button>
</template>
