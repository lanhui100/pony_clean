<script setup lang="ts">
import { type HTMLAttributes } from 'vue'
import { cn } from '../../../lib/utils'

interface Props {
  variant?: 'default' | 'destructive'
  class?: HTMLAttributes['class']
}

const props = withDefaults(defineProps<Props>(), {
  variant: 'default',
})
</script>

<template>
  <div
    role="alert"
    :class="cn(
      'relative w-full rounded-lg border p-4',
      variant === 'default' && 'bg-background text-foreground',
      variant === 'destructive' && 'border-destructive/50 text-destructive dark:border-destructive',
      props.class,
    )"
  >
    <slot />
  </div>
</template>
