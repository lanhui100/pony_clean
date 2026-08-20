<script setup lang="ts">
import { ref, watch, onUnmounted } from 'vue'
import { writeText } from '@tauri-apps/plugin-clipboard-manager'
import { Check, Copy, X } from 'lucide-vue-next'

interface Props {
  show: boolean
  message: string
  /** 原始错误信息（复制按钮使用）；错误提示时建议传入 */
  rawError?: string
  variant?: 'success' | 'error'
  /** 成功提示自动消失时长（ms）；错误提示保持到用户处理完 */
  duration?: number
}

const props = withDefaults(defineProps<Props>(), {
  rawError: '',
  variant: 'success',
  duration: 4000,
})

const emit = defineEmits<{ close: [] }>()

const copied = ref(false)
let autoTimer: ReturnType<typeof setTimeout> | null = null
let copyTimer: ReturnType<typeof setTimeout> | null = null

watch(
  () => props.show,
  (v) => {
    if (autoTimer) clearTimeout(autoTimer)
    autoTimer = null
    if (v && props.variant === 'success') {
      autoTimer = setTimeout(() => emit('close'), props.duration)
    }
  },
  { immediate: true },
)

/** 一键复制原始错误信息到剪贴板（优先 Tauri 插件，失败回退 Web API） */
async function copyError() {
  if (!props.rawError) return
  try {
    await writeText(props.rawError)
  } catch {
    try {
      await navigator.clipboard.writeText(props.rawError)
    } catch {
      return
    }
  }
  copied.value = true
  if (copyTimer) clearTimeout(copyTimer)
  copyTimer = setTimeout(() => { copied.value = false }, 1500)
}

onUnmounted(() => {
  if (autoTimer) clearTimeout(autoTimer)
  if (copyTimer) clearTimeout(copyTimer)
})
</script>

<template>
  <Transition name="toast-fade">
    <div
      v-if="show"
      class="fixed bottom-3 left-1/2 z-50 flex max-w-[90%] -translate-x-1/2 flex-col gap-1 rounded-md px-2.5 py-1.5 text-[11px] font-medium shadow-lg backdrop-blur-md"
      :class="variant === 'success' ? 'bg-success/60 text-white' : 'bg-destructive/60 text-white'"
    >
      <div class="flex items-center gap-1.5">
        <span class="min-w-0 truncate" :title="message">{{ message }}</span>
        <template v-if="variant === 'error'">
          <button
            class="flex h-4 w-4 shrink-0 items-center justify-center rounded-sm transition-colors hover:bg-white/20"
            :title="copied ? '已复制' : '复制错误信息'"
            :aria-label="copied ? '已复制' : '复制错误信息'"
            @click.stop="copyError"
          >
            <Check v-if="copied" class="h-3 w-3" />
            <Copy v-else class="h-3 w-3" />
          </button>
          <button
            class="flex h-4 w-4 shrink-0 items-center justify-center rounded-sm transition-colors hover:bg-white/20"
            title="关闭"
            aria-label="关闭提示"
            @click="emit('close')"
          >
            <X class="h-3 w-3" />
          </button>
        </template>
      </div>
      <slot />
    </div>
  </Transition>
</template>

<style scoped>
.toast-fade-enter-active,
.toast-fade-leave-active {
  transition: opacity 0.2s ease, transform 0.2s ease;
}
.toast-fade-enter-from,
.toast-fade-leave-to {
  opacity: 0;
  transform: translate(-50%, 6px);
}
</style>