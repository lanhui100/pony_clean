<script setup lang="ts">
import { type HTMLAttributes } from 'vue'
import { badgeVariants, type BadgeVariants } from '.'

interface Props {
  variant?: BadgeVariants['variant']
  class?: HTMLAttributes['class']
}

const props = withDefaults(defineProps<Props>(), {
  variant: 'default',
})
</script>

<template>
  <div :class="badgeVariants({ variant: props.variant, class: props.class })">
    <slot />
  </div>
</template>
