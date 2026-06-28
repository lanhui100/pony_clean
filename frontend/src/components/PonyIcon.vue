<script setup lang="ts">
const props = withDefaults(defineProps<{ size?: number }>(), { size: 16 })
</script>

<template>
  <svg
    :width="props.size"
    :height="props.size"
    viewBox="0 0 24 24"
    fill="none"
    xmlns="http://www.w3.org/2000/svg"
    class="text-amber-400"
  >
    <path
      d="M12 3a9 9 0 00-9 9v1a9 9 0 0018 0v-1a9 9 0 00-9-9z"
      stroke="currentColor"
      stroke-width="1.5"
      fill="currentColor"
      fill-opacity="0.2"
    />
    <path
      d="M8.5 13a3.5 3.5 0 007 0"
      stroke="currentColor"
      stroke-width="1.5"
      stroke-linecap="round"
    />
    <circle cx="9.5" cy="9" r="1" fill="currentColor" fill-opacity="0.8" />
    <circle cx="14.5" cy="9" r="1" fill="currentColor" fill-opacity="0.8" />
  </svg>
</template>
