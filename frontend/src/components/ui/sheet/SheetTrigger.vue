<script setup lang="ts">
import { inject } from 'vue'

const { open } = inject<any>('sheetContext')

function openSheet() {
  open.value = true
}
</script>

<template>
  <span @click="openSheet" class="inline-flex cursor-pointer">
    <slot />
  </span>
</template>
