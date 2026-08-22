<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { motion } from 'motion-v'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import CapsuleBar from '@/components/CapsuleBar.vue'
import EdgeBar from '@/components/EdgeBar.vue'
import { useMonitor } from '@/composables/useMonitor'
import { useWindowMorph } from '@/composables/useWindowMorph'
import { contentRectFor } from '@/lib/windowMorphConfig'

const isScanning = ref(false)
let unlistenScanState: UnlistenFn | null = null
let unlistenResetPos: UnlistenFn | null = null
let unlistenCollapseRequest: UnlistenFn | null = null
const { cpuPercent, memPercent, setPollInterval } = useMonitor()
const {
  islandState,
  capsuleHovered,
  form,
  onCapsuleEnter,
  onCapsuleLeave,
  onBarEnter,
  onBarLeave,
  onCapsuleDragStart,
  onCapsuleClick,
  onEnterDone,
  onLeaveDone,
  notifyActivity,
  resetToDefault,
  hideIsland,
} = useWindowMorph(isScanning)

// 胶囊层 / 进度条层的内容矩形（窗口内 CSS 像素，横排）
const pillRect = computed(() => contentRectFor('pill'))
const barRect = computed(() => contentRectFor('bar'))

// 形态变换（SPEC-029）：出层只淡出（位置不变），入层从另一形态的矩形
// morph 到自身矩形 —— 单方向空间变化，杜绝双向交叉淡入的叠影残影。
// transform-origin 0 0，CSS Transition（见 style 块），时长为
// WINDOW_MORPH.morphDurationMs，与 useWindowMorph 的延迟原生几何同步对齐。
const pillLayerStyle = computed(() => {
  const p = pillRect.value
  const b = barRect.value
  return {
    left: `${p.x}px`,
    top: `${p.y}px`,
    width: `${p.w}px`,
    height: `${p.h}px`,
    // enter-from：pill 层从 bar 矩形 morph 而来
    '--from-x': `${b.x - p.x}px`,
    '--from-y': `${b.y - p.y}px`,
    '--from-sx': (b.w / p.w).toString(),
    '--from-sy': (b.h / p.h).toString(),
    // 圆角形状随 morph 同步过渡（--shape 由 Transition 类在首帧后翻转，
    // 子层 CapsuleBar 用 var(--shape) 渐变圆角，缩放中的轮廓始终贴合目标形态）
    '--shape-from': '0 0 9999px 9999px',
    '--shape-to': '9999px',
  }
})

const barLayerStyle = computed(() => {
  const p = pillRect.value
  const b = barRect.value
  return {
    left: `${b.x}px`,
    top: `${b.y}px`,
    width: `${b.w}px`,
    height: `${b.h}px`,
    // enter-from：bar 层从 pill 矩形 morph 而来
    '--from-x': `${p.x - b.x}px`,
    '--from-y': `${p.y - b.y}px`,
    '--from-sx': (p.w / b.w).toString(),
    '--from-sy': (p.h / b.h).toString(),
    // 圆角形状随 morph 过渡：从胶囊全圆角渐变为「贴边侧方角 + 远端半圆」
    '--shape-from': '9999px',
    '--shape-to': '0 0 9999px 9999px',
  }
})

const prefersReducedMotion =
  typeof window !== 'undefined' &&
  window.matchMedia('(prefers-reduced-motion: reduce)').matches

// island 展开/收起时的整体淡出缩放（仅包裹层，pill/bar 的 complete 事件不再污染此回调）
const capsuleAnimate = computed(() => {
  if (islandState.value === 'idle' || islandState.value === 'leaving') return { opacity: 1, scale: 1 }
  return { opacity: 0, scale: 0.9 }
})

const capsuleTransition = computed(() => {
  if (prefersReducedMotion) return { duration: 0.001 }
  return {
    type: 'spring' as const,
    stiffness: 220,
    damping: 18,
    mass: 0.75,
  }
})

watch(islandState, () => {
  setPollInterval(islandState.value === 'visible' ? 2000 : 3000)
})

// 扫描状态变化时刷新收起计时（扫描中不收起为进度条）
watch(isScanning, () => {
  notifyActivity()
})

onMounted(async () => {
  unlistenScanState = await listen<{ scanning: boolean }>('scan-state-changed', (e) => {
    isScanning.value = e.payload?.scanning ?? false
  }).catch(() => null)

  // 托盘菜单“重置胶囊位置”：重置到顶边居中
  unlistenResetPos = await listen('reset-capsule-position', () => {
    resetToDefault()
  }).catch(() => null)

  // 岛屿面板“收起到胶囊”按钮：扫描期间空转检测被挂起，此入口让用户手动收起，
  // 长时扫描不遮蔽其他窗口（扫描在后台继续，重新点开胶囊即恢复面板）
  unlistenCollapseRequest = await listen('island-collapse-request', () => {
    hideIsland()
  }).catch(() => null)

  // 渲染自检：挂载后检查胶囊层是否真的可见，结果转发到 Rust 终端
  setTimeout(() => {
    window.__ponyLog?.('info', `render-check: form=${form.value}`)
    const el = document.querySelector<HTMLElement>('[data-pill-layer]')
    if (!el) {
      window.__ponyLog?.('error', 'render-check: pill layer element not found (template render failed?)')
      return
    }
    const cs = window.getComputedStyle(el)
    const pill = pillRect.value
    window.__ponyLog?.(
      'info',
      `render-check: pillLayer opacity=${cs.opacity} display=${cs.display} ` +
        `pos=${cs.left},${cs.top} size=${cs.width}x${cs.height} z=${cs.zIndex} ` +
        `expected=${pill.x},${pill.y} ${pill.w}x${pill.h}`,
    )
  }, 1200)
})

onUnmounted(() => {
  unlistenScanState?.()
  unlistenResetPos?.()
  unlistenCollapseRequest?.()
})

/**
 * 仅由最外层 motion.div（island 淡出层）的 complete 触发。
 * 注意：pill/bar 两层是【纯 CSS Transition】，不派发 motion-v 的 complete；
 * 本回调与 pill⇄bar 形态切换完全解耦（SPEC-029 裁决：complete 回调不得共享），
 * 请勿在 pill/bar 层改用 motion 时不经拆分直接复用此回调。
 */
function onIslandFadeComplete() {
  if (islandState.value === 'entering') onEnterDone()
  else if (islandState.value === 'leaving') onLeaveDone()
}
</script>

<template>
  <div class="capsule-root h-screen w-screen overflow-hidden select-none" @mousedown="onCapsuleDragStart">
    <motion.div
      class="island-fade-layer"
      :animate="capsuleAnimate"
      :transition="capsuleTransition"
      :on-animation-complete="onIslandFadeComplete"
    >
      <!-- 胶囊层（form=pill 时挂载；进入时从 bar 矩形 morph 而来，离开时仅淡出） -->
      <Transition name="morph-pill">
        <div
          v-if="form === 'pill'"
          class="content-layer"
          data-pill-layer
          :style="pillLayerStyle"
          @mouseenter="onCapsuleEnter"
          @mouseleave="onCapsuleLeave"
          @click="onCapsuleClick"
        >
          <CapsuleBar
            :cpu-percent="cpuPercent"
            :mem-percent="memPercent"
            :is-hovered="capsuleHovered"
          />
        </div>
      </Transition>

      <!-- 贴边进度条层（form=bar 时挂载；进入时从 pill 矩形 morph 而来，离开时仅淡出） -->
      <Transition name="morph-bar">
        <div
          v-if="form === 'bar'"
          class="content-layer"
          :style="barLayerStyle"
          @mouseenter="onBarEnter"
          @mouseleave="onBarLeave"
          @click="onCapsuleClick"
        >
          <EdgeBar
            :cpu-percent="cpuPercent"
            :mem-percent="memPercent"
          />
        </div>
      </Transition>
    </motion.div>
  </div>
</template>

<style scoped>
.capsule-root {
  background: transparent;
  position: relative;
}

.island-fade-layer {
  position: absolute;
  inset: 0;
  z-index: 10;
  pointer-events: auto;
  will-change: transform, opacity;
}

.content-layer {
  position: absolute;
  z-index: 20;
  cursor: grab;
  will-change: transform, opacity;
  transform-origin: 0 0;
  /* 静止/入层终态的圆角形状（子层通过 var(--shape) 继承） */
  --shape: var(--shape-to);
}

.content-layer:active {
  cursor: grabbing;
}

/* ─── pill ⇄ bar morph（SPEC-029）───
   入层：从另一形态矩形 translate+scale 到自身 + 淡入（300ms，与
   useWindowMorph 的延迟原生几何同步对齐）；出层：仅 160ms 淡出，不做空间变换。
   两端 rect 通过 CSS 变量（--from-*）注入，无需 JS 动画回调。
   圆角形状（--shape）同样在首帧取来源形态、随后翻转为目标形态，子层
   （CapsuleBar/EdgeBar）以同曲线的 border-radius transition 跟随渐变，
   使缩放中的轮廓与目标形态一致，避免端部圆角畸变。 */
.morph-pill-enter-from,
.morph-bar-enter-from {
  transform: translate(var(--from-x), var(--from-y)) scale(var(--from-sx), var(--from-sy));
  opacity: 0;
  --shape: var(--shape-from);
}
.morph-pill-enter-active,
.morph-bar-enter-active {
  transition:
    transform 300ms cubic-bezier(0.22, 1, 0.36, 1),
    opacity 220ms ease-out;
}
.morph-pill-enter-to,
.morph-bar-enter-to {
  transform: translate(0, 0) scale(1, 1);
  opacity: 1;
}
.morph-pill-leave-from,
.morph-bar-leave-from {
  opacity: 1;
}
.morph-pill-leave-active,
.morph-bar-leave-active {
  transition: opacity 160ms ease-in;
}
.morph-pill-leave-to,
.morph-bar-leave-to {
  opacity: 0;
}
</style>
