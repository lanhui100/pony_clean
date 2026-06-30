<script setup lang="ts">
import { inject } from 'vue'

const { close } = inject<any>('sheetContext')
</script>

<template>
  <span @click="close" class="inline-flex cursor-pointer">
    <slot />
  </span>
</template>
