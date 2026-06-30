<script setup lang="ts">
import { ref, provide } from 'vue'

const open = ref(false)

function close() {
  open.value = false
}

provide('sheetContext', { open, close })
</script>

<template>
  <slot :open="open" :close="close" />
</template>
