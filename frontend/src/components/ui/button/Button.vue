<script setup lang="ts">
import { type HTMLAttributes, computed } from 'vue'
import { buttonVariants, type ButtonVariants } from '.'

interface Props {
  variant?: ButtonVariants['variant']
  size?: ButtonVariants['size']
  disabled?: boolean
  class?: HTMLAttributes['class']
}

const props = withDefaults(defineProps<Props>(), {
  variant: 'default',
  size: 'default',
})

const emit = defineEmits<{ click: [e: MouseEvent] }>()

const classes = computed(() => buttonVariants({ variant: props.variant, size: props.size, class: props.class }))
</script>

<template>
  <button :class="classes" :disabled="disabled" @click="emit('click', $event)">
    <slot />
  </button>
</template>
