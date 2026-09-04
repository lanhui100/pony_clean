<script setup lang="ts">
import { computed } from 'vue'
import type { NetQuality } from '@/composables/useMonitor'

/**
 * 网络状态指示器（SPEC-032）
 *
 * 颜色沿用项目现有三族（与 CapsuleBar/EdgeBar 的 tint 一致）：
 * good → green · poor → amber · offline → red-500 · null（检测中）→ white 系。
 * dot 形态用 600 系实色；hairline 线宽仅 3px，改用 500 系提亮保证存在感。
 *
 * - `variant="dot"`（缺省）：点状。胶囊内使用不传 glow（overflow:hidden 裁剪外发光，
 *   仅保留 inset 内描边高光）；面板 NET 行内可开 glow。
 * - `variant="hairline"`：3px 发丝分隔线（2026-09-04 修订，替代中央 6px 柱状）。
 *   「状态即分隔线」：分隔线本身用状态色着色，兼作 CPU/MEM 分隔边界与网络状态表达；
 *   文档流内 flex 子元素，morph 随容器缩放天然连续（无需绝对定位补丁），
 *   调用方加 `self-center` 使其在父级交叉轴居中；
 *   检测中态 white/20 保证分隔永不消失。贴边条态 hover 即展开为胶囊、
 *   tooltip 不可达，故调用方传 titled=false 且不拦截指针。
 * - hairline 长度/明暗随实时流量动态变化（2026-09-04 修订二，`downBps/upBps`）：
 *   长度 = 总速率对数映射（40%→80% 高度，CSS 过渡平滑）；颜色保持质量色系不变、
 *   仅不透明度随活跃度呼吸（静默变暗、忙时提亮）——颜色已承载质量语义，
 *   不再叠加速度语义以免歧义。tooltip 在线时追加 ↓/↑ 速率。
 * - poor/offline 态呼吸脉动。
 * - 组件自带 prefers-reduced-motion 关闭动画（全局兜底不含 iteration-count）。
 * - 注意：颜色来自滞回滤波后的门控状态，title 中的延迟为当轮原始值——
 *   过渡期间两者可能来自不同轮次，属滞回设计固有（代码审核记录项）。
 */
const props = withDefaults(
  defineProps<{
    status: NetQuality | null
    latencyMs?: number | null
    /** 下行速率 B/s（null = 未知；在线时 tooltip 显示，hairline 计入长度映射） */
    downBps?: number | null
    /** 上行速率 B/s（同上） */
    upBps?: number | null
    /** 形态：dot 圆点（缺省）/ hairline 发丝分隔线 */
    variant?: 'dot' | 'hairline'
    /** 外发光（仅在无 overflow:hidden 裁剪的容器内启用；仅 dot 形态生效） */
    glow?: boolean
    /** 直径 px，缺省 6，负值按 0 处理（仅 dot 形态生效） */
    size?: number
    /** 是否显示 hover title 提示 */
    titled?: boolean
  }>(),
  {
    latencyMs: null,
    downBps: null,
    upBps: null,
    variant: 'dot',
    glow: false,
    size: 6,
    titled: true,
  },
)

const isHairline = computed(() => props.variant === 'hairline')

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

/** hairline 形态：2px 线宽需实色才有存在感（检测中 white/20 保证分隔永不消失） */
const colorHairlineClass = computed(() => {
  switch (props.status) {
    case 'good':
      return 'bg-green-500/80'
    case 'poor':
      return 'bg-amber-500/85'
    case 'offline':
      return 'bg-red-500/85'
    default:
      return 'bg-white/20'
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
      return `网络良好 · ${props.latencyMs ?? '—'}ms${rateSuffix.value} · ${scope}`
    case 'poor':
      return `网络较差 · ${props.latencyMs ?? '—'}ms${rateSuffix.value} · ${scope}`
    case 'offline':
      return `网络已断开 · ${scope}`
    default:
      return '网络检测中…'
  }
})

/**
 * hairline 流量等级 0→1（2026-09-04 修订二）。
 * 总速率对数映射：静默 ≈0、后台涓流 ~0.4–0.5、浏览突发 ~0.7–0.85、下载封顶 1；
 * 双 null（旧后端缺字段）取中性 0.75。线性映射会被下载峰值压扁日常区间，故用对数。
 */
const LEVEL_REF_BPS = 1024 * 1024 // 1MB/s 封顶基准
const level = computed(() => {
  const d = props.downBps
  const u = props.upBps
  if (d == null && u == null) return 0.75
  const total = Math.max(0, (d ?? 0) + (u ?? 0))
  return Math.min(1, Math.log10(1 + total) / Math.log10(1 + LEVEL_REF_BPS))
})

/** 长度 40%→80% 高度（胶囊约 16→32px，贴边条约 4→8px）；明暗静默变暗、忙时提亮 */
const hairlineStyle = computed(() => ({
  height: `${Math.round(40 + level.value * 40)}%`,
  opacity: props.status == null ? 1 : Number((0.55 + 0.45 * level.value).toFixed(2)),
}))

/** 速率格式化：B/s → KB/s → MB/s；null/非有限 →「—」（与 MonitorPanel.speedParts 同口径） */
function fmtBps(bps: number | null): string {
  if (bps == null || !Number.isFinite(bps)) return '—'
  if (bps < 1024) return `${Math.round(bps)}B/s`
  const kb = bps / 1024
  if (kb < 1024) return `${kb.toFixed(1)}KB/s`
  return `${(kb / 1024).toFixed(1)}MB/s`
}

/** 在线时 tooltip 追加的 ↓/↑ 速率段；offline 速率是全接口合计、与质量口径不同，不混显 */
const rateSuffix = computed(() => {
  if (props.status !== 'good' && props.status !== 'poor') return ''
  if (props.downBps == null && props.upBps == null) return ''
  return ` · ↓${fmtBps(props.downBps)} ↑${fmtBps(props.upBps)}`
})

const pulsing = computed(() => props.status === 'poor' || props.status === 'offline')
</script>

<template>
  <!-- hairline 形态：3px 发丝分隔线，状态色即分隔线色，兼作 CPU/MEM 分隔；
       高度/明暗由 hairlineStyle 按实时流量驱动（CSS 过渡平滑）；
       点击冒泡触发展开属预期（贴边条态调用方负责 pointer-events-none） -->
  <span
    v-if="isHairline"
    class="net-dot net-dot-hairline inline-block w-[3px] shrink-0 self-center rounded-full"
    :class="[colorHairlineClass, { 'net-dot-pulse': pulsing }]"
    :style="hairlineStyle"
    :title="title"
  />
  <span
    v-else
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

/* hairline 形态无边框无外发光：内描边高光在 3px 线上会吞掉本色，故覆盖去掉；
   高度/明暗由行内样式按流量驱动，此处过渡保证轮询步进间平滑伸缩 */
.net-dot-hairline {
  box-shadow: none;
  transition:
    height 0.7s ease,
    opacity 0.7s ease;
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
  .net-dot-hairline {
    transition: none;
  }
}
</style>
