<script setup lang="ts">
import { getCurrentWindow } from '@tauri-apps/api/window'
import CapsuleWindow from '@/components/CapsuleWindow.vue'
import IslandWindow from '@/components/IslandWindow.vue'

const windowLabel = getCurrentWindow().label
const isCapsuleWindow = windowLabel === 'capsule' || windowLabel === 'main'
</script>

<template>
  <CapsuleWindow v-if="isCapsuleWindow" />
  <IslandWindow v-else />
</template>
