<script setup lang="ts">
import { ref, onMounted, watch, nextTick } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import TitleBar from '@/components/TitleBar.vue'
import CapsuleBar from '@/components/CapsuleBar.vue'
import CleanerPanel from '@/views/CleanerPanel.vue'
import MonitorPanel from '@/views/MonitorPanel.vue'
import { useMonitor } from '@/composables/useMonitor'
import { useWindowMorph } from '@/composables/useWindowMorph'

const activeTab = ref('monitor')
const searchQuery = ref('')
const isScanning = ref(false)

const { cpuPercent, memPercent, setPollInterval } = useMonitor()
const {
  morphState, showCapsule, isFirstDock,
  onUserActivity, onCapsuleHover, onCapsuleDragStart, onCapsuleClick,
  onShrinkAnimEnd, onExpandAnimEnd, setDockPref, dockSide, userDockPref,
} = useWindowMorph(isScanning)

const panelReady = ref(false)

watch(morphState, async (state) => {
  setPollInterval(state === 'full' ? 2000 : 3000)
  if (state === 'expanding') {
    // Mount panel in hidden state, then trigger transition
    await nextTick()
    panelReady.value = true
  } else {
    panelReady.value = false
  }
})

onMounted(async () => {
  try {
    await getCurrentWindow().setEffects({
      effects: [{ effect: 'acrylic' }],
      color: { r: 30, g: 28, b: 26, a: 200 },
    })
  } catch (e) {
    console.warn('[PonyClean] window effects not supported:', e)
  }
})

function onFullWindowMouseMove() { onUserActivity() }
function onFullWindowMouseDown() { onUserActivity() }
function onFullWindowKeyDown() { onUserActivity() }
</script>

<template>
  <div
    :class="[
      'root h-screen w-screen overflow-hidden',
      morphState === 'shrinking' ? 'is-shrinking' : '',
      morphState === 'capsule' || morphState === 'docking' || morphState === 'docked' || morphState === 'expanding' ? 'is-capsule' : '',
      morphState === 'expanding' ? 'is-expanding' : '',
    ]"
  >
    <!-- Capsule layer: centered in window for smooth shrink/expand -->
    <div
      v-if="showCapsule"
      class="capsule-layer"
      @mouseenter="onCapsuleHover"
      @mousedown="onCapsuleDragStart"
    >
      <CapsuleBar
        :cpu-percent="cpuPercent"
        :mem-percent="memPercent"
        :is-first-dock="isFirstDock"
        @click="onCapsuleClick"
      />
    </div>

    <!-- Full panel layer -->
    <div
      v-if="morphState === 'full' || morphState === 'shrinking' || (morphState === 'expanding')"
      class="panel-layer"
      :class="(morphState === 'full' || (morphState === 'expanding' && panelReady)) ? 'panel-visible' : 'panel-hidden'"
      @transitionend="morphState === 'shrinking' ? onShrinkAnimEnd() : (morphState === 'expanding' ? onExpandAnimEnd() : null)"
      @mousemove="onFullWindowMouseMove"
      @mousedown="onFullWindowMouseDown"
      @keydown="onFullWindowKeyDown"
    >
      <div class="flex h-full w-full flex-col text-foreground" style="background: linear-gradient(145deg, hsl(30,12%,9%) 0%, hsl(30,12%,14%) 40%, hsl(28,10%,18%) 70%, hsl(25,8%,22%) 100%)">
        <TitleBar
          v-model:activeTab="activeTab"
          v-model:searchQuery="searchQuery"
          :morph-state="morphState"
          :dock-side="dockSide"
          :user-dock-pref="userDockPref"
          @update:dockPref="setDockPref"
        />
        <main class="flex-1 overflow-hidden p-4 pt-2">
          <div class="h-full">
            <KeepAlive>
              <MonitorPanel v-if="activeTab === 'monitor'" key="monitor" :search="searchQuery" />
              <CleanerPanel v-else key="cleaner" @scan-start="isScanning = true" @scan-end="isScanning = false" />
            </KeepAlive>
          </div>
        </main>
      </div>
    </div>
  </div>
</template>

<style scoped>
.root {
  background: transparent;
  position: relative;
}

/* ─── Panel ─── */
.panel-layer {
  position: absolute;
  inset: 0;
  z-index: 1;
  transition: transform 400ms cubic-bezier(0.32, 0.72, 0, 1),
              opacity 400ms cubic-bezier(0.32, 0.72, 0, 1);
}
.panel-visible {
  transform: scale(1);
  opacity: 1;
}
.panel-hidden {
  transform: scale(0.3);
  opacity: 0;
  pointer-events: none;
}

/* ─── Capsule (centered) ─── */
.capsule-layer {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%) scale(1);
  width: 160px;
  height: 40px;
  z-index: 2;
  opacity: 1;
  pointer-events: auto;
}

/* ─── Shrinking: capsule enters from scaled center ─── */
.is-shrinking .capsule-layer {
  animation: capsule-enter 300ms cubic-bezier(0.32, 0.72, 0, 1) 200ms both;
}

@keyframes capsule-enter {
  from {
    transform: translate(-50%, -50%) scale(0.3);
    opacity: 0;
  }
  to {
    transform: translate(-50%, -50%) scale(1);
    opacity: 1;
  }
}

/* ─── Expanding: capsule exits, panel enters ─── */
.is-expanding .capsule-layer {
  animation: capsule-exit 200ms cubic-bezier(0.32, 0.72, 0, 1) both;
}

@keyframes capsule-exit {
  from {
    transform: translate(-50%, -50%) scale(1);
    opacity: 1;
  }
  to {
    transform: translate(-50%, -50%) scale(0.3);
    opacity: 0;
  }
}
</style>