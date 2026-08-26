<script setup lang="ts">
import { computed } from 'vue'
import NetDot from '@/components/NetDot.vue'
import type { NetQuality } from '@/composables/useMonitor'

const props = defineProps<{
  cpuPercent: number
  memPercent: number
  isHovered?: boolean
  /** 公网连通质量（null = 检测中） */
  netStatus?: NetQuality | null
  netLatencyMs?: number | null
}>()

const emit = defineEmits<{
  (e: 'click'): void
}>()

const cpuFill = computed(() => ({ width: `${Math.min(props.cpuPercent, 100)}%` }))
const memFill = computed(() => ({ width: `${Math.min(props.memPercent, 100)}%` }))

const cpuTint = computed(() => {
  const p = props.cpuPercent
  if (p >= 80) return 'bg-red-500/75'
  if (p >= 50) return 'bg-amber-600/75'
  return 'bg-green-600/75'
})

const cpuBg = computed(() => {
  const p = props.cpuPercent
  if (p >= 80) return 'bg-red-500/12'
  if (p >= 50) return 'bg-amber-600/12'
  return 'bg-green-600/12'
})

const memTint = computed(() => {
  const p = props.memPercent
  if (p >= 80) return 'bg-red-500/75'
  if (p >= 50) return 'bg-amber-600/75'
  return 'bg-green-600/75'
})

const memBg = computed(() => {
  const p = props.memPercent
  if (p >= 80) return 'bg-red-500/12'
  if (p >= 50) return 'bg-amber-600/12'
  return 'bg-green-600/12'
})
</script>

<template>
  <div
    class="capsule-bar relative flex cursor-pointer items-center overflow-hidden select-none h-full w-full transition-shadow duration-300"
    :class="{ 'capsule-hovered': isHovered }"
    @click="emit('click')"
  >
    <!-- CPU half -->
    <div class="relative flex flex-1 items-center justify-center h-full overflow-hidden rounded-l-full" :class="cpuBg">
      <div
        class="progress-fill absolute left-0 top-0 h-full rounded-l-full"
        :class="cpuTint"
        :style="cpuFill"
      />
      <span class="relative z-10 text-white font-bold text-[13px] leading-none drop-shadow-sm">
        {{ cpuPercent }}<span class="text-white/60 text-[10px] ml-0.5">CPU</span>
      </span>
    </div>

    <!-- 中央网络状态竖向指示条（2026-08-26 用户反馈修订，替代原中央圆点+1px 分隔线）：
         兼作 CPU/MEM 分隔，左右浅边线即边界；20px 居中沿用旧分隔线高度；
         文档流内 flex 子元素，morph 随容器缩放天然连续（无需绝对定位补丁）；
         实心色芯无外发光（overflow:hidden 裁剪），点击冒泡触发展开属预期 -->
    <NetDot
      variant="bar"
      class="h-5"
      :status="netStatus ?? null"
      :latency-ms="netLatencyMs"
    />

    <!-- MEM half -->
    <div class="relative flex flex-1 items-center justify-center h-full overflow-hidden rounded-r-full" :class="memBg">
      <div
        class="progress-fill absolute right-0 top-0 h-full rounded-r-full"
        :class="memTint"
        :style="memFill"
      />
      <span class="relative z-10 text-white font-bold text-[13px] leading-none drop-shadow-sm">
        {{ memPercent }}<span class="text-white/60 text-[10px] ml-0.5">MEM</span>
      </span>
    </div>
  </div>
</template>

<style scoped>
.capsule-bar {
  background:
    linear-gradient(180deg, rgba(42, 39, 35, 0.98), rgba(20, 19, 18, 0.96));
  border: 0;
  /* 胶囊四角全圆；圆角随父层 morph 过渡（--shape 由 CapsuleWindow 注入，
     从 bar 的「贴边侧方角」形状渐变而来），缺省即全圆角胶囊 */
  border-radius: var(--shape, 9999px);
  transition:
    border-radius 300ms cubic-bezier(0.22, 1, 0.36, 1),
    box-shadow 0.25s ease,
    border-color 0.25s ease;
  /* 顶部内高光；外阴影由原生 DWM（CS_DROPSHADOW 按胶囊 Region）提供
     （SPEC-029 二次修订，面板即窗口，无 CSS 阴影边距） */
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.08);
}

.progress-fill {
  opacity: 0.72;
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.10);
}

.capsule-hovered {
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.12),
    0 0 14px rgba(255, 255, 255, 0.07),
    0 0 28px rgba(255, 255, 255, 0.04);
}
</style>
