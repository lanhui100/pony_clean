<script setup lang="ts">
import { inject } from 'vue'
import { cn } from '../../../lib/utils'
import type { Ref } from 'vue'

interface Props {
  class?: string
}

const props = defineProps<Props>()
const { isOpen } = inject<{ toggle: () => void; isOpen: Ref<boolean> }>('collapsible')!
</script>

<template>
  <div v-if="isOpen" :class="cn('overflow-hidden', props.class)">
    <slot />
  </div>
</template>
