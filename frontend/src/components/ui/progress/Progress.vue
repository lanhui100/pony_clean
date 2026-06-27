<script setup lang="ts">
import { type HTMLAttributes } from 'vue'
import { cn } from '../../../lib/utils'

interface Props {
  modelValue?: number | null
  class?: HTMLAttributes['class']
}

const props = withDefaults(defineProps<Props>(), {
  modelValue: null,
})
</script>

<template>
  <div
    role="progressbar"
    :class="cn('relative h-2 w-full overflow-hidden rounded-full bg-secondary', props.class)"
  >
    <div
      v-if="modelValue !== null"
      class="h-full w-full flex-1 bg-primary transition-all duration-500"
      :style="{ transform: `translateX(-${100 - (modelValue ?? 0)}%)` }"
    />
    <div
      v-else
      class="h-full w-full"
    >
      <div
        class="animate-progress h-full w-1/3 rounded-full bg-primary"
      />
    </div>
  </div>
</template>
