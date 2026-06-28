<script setup lang="ts">
import iconUrl from '@/assets/pony-icon.png'

withDefaults(defineProps<{ size?: number }>(), { size: 16 })
</script>

<template>
  <img
    :width="size"
    :height="size"
    :src="iconUrl"
    alt=""
    aria-hidden="true"
    class="shrink-0"
  />
</template>
