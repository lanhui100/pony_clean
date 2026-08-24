<script setup lang="ts">
import { computed } from 'vue'
import type { NetQuality } from '@/composables/useMonitor'

/**
 * 网络状态点状指示器（SPEC-032）
 *
 * 颜色沿用项目现有三族（与 CapsuleBar/EdgeBar 的 tint 一致）：
 * good → green-600 · poor → amber-600 · offline → red-500 · null（检测中）→ white/25。
 *
 * - 胶囊/贴边内使用：不传 glow（两层根容器 overflow:hidden 会裁剪外发光，
 *   仅保留 inset 内描边高光）；poor/offline 态呼吸脉动。
 * - 面板 NET 行内使用：可开 glow（无裁剪上下文）。
 * - 组件自带 prefers-reduced-motion 关闭动画（全局兜底不含 iteration-count）。
 * - 注意：颜色来自滞回滤波后的门控状态，title 中的延迟为当轮原始值——
 *   过渡期间两者可能来自不同轮次，属滞回设计固有（代码审核记录项）。
 */
const props = withDefaults(
  defineProps<{
    status: NetQuality | null
    latencyMs?: number | null
    /** 外发光（仅在无 overflow:hidden 裁剪的容器内启用） */
    glow?: boolean
    /** 直径 px，缺省 6，负值按 0 处理 */
    size?: number
    /** 是否显示 hover title 提示 */
    titled?: boolean
  }>(),
  {
    latencyMs: null,
    glow: false,
    size: 6,
    titled: true,
  },
)

const colorClass = computed(() => {
  switch (props.status) {
    case 'good':
      return 'bg-green-600'
    case 'poor':
      return 'bg-amber-600'
    case 'offline':
      return 'bg-red-500'
    default:
      return 'bg-white/25'
  }
})

const dotStyle = computed(() => {
  const size = Math.max(0, props.size)
  return { width: `${size}px`, height: `${size}px` }
})

const title = computed(() => {
  if (!props.titled) return undefined
  // 探测语义为「公网连通性」而非「互联网可用性」（captive portal 假阳见 SPEC §2）
  const scope = '公网连通性'
  switch (props.status) {
    case 'good':
      return `网络良好 · ${props.latencyMs ?? '—'}ms · ${scope}`
    case 'poor':
      return `网络较差 · ${props.latencyMs ?? '—'}ms · ${scope}`
    case 'offline':
      return `网络已断开 · ${scope}`
    default:
      return '网络检测中…'
  }
})

const pulsing = computed(() => props.status === 'poor' || props.status === 'offline')
</script>

<template>
  <span
    class="net-dot inline-block shrink-0 rounded-full"
    :class="[colorClass, { 'net-dot-glow': glow, 'net-dot-pulse': pulsing }]"
    :style="dotStyle"
    :title="title"
  />
</template>

<style scoped>
.net-dot {
  /* 内描边高光：不被容器 overflow:hidden 裁剪（外发光在胶囊/贴边内必裁，故缺省不用） */
  box-shadow: inset 0 0 0.5px rgba(255, 255, 255, 0.35);
}

.net-dot-glow {
  box-shadow:
    0 0 6px rgba(255, 255, 255, 0.22),
    inset 0 0 0.5px rgba(255, 255, 255, 0.35);
}

/* poor/offline 呼吸脉动；组件级 reduced-motion 直接关闭 */
.net-dot-pulse {
  animation: net-dot-pulse 2s ease-in-out infinite;
}

@keyframes net-dot-pulse {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.45;
  }
}

@media (prefers-reduced-motion: reduce) {
  .net-dot-pulse {
    animation: none;
  }
}
</style>
