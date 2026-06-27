<script setup lang="ts">
import { ref, provide, type Ref } from 'vue'

interface Props {
  open?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  open: false,
})

const emit = defineEmits<{ 'update:open': [value: boolean] }>()

const isOpen = ref(props.open)

function toggle() {
  isOpen.value = !isOpen.value
  emit('update:open', isOpen.value)
}

provide('collapsible', { isOpen, toggle })
</script>

<template>
  <div>
    <slot :open="isOpen" />
  </div>
</template>
