<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { writeText } from '@tauri-apps/plugin-clipboard-manager'
import { AppWindow, Check, Copy, Loader2, RefreshCw, X } from 'lucide-vue-next'
import { Button } from '../components/ui/button'
import { Switch } from '../components/ui/switch'
import { humanizeInvokeError } from '../lib/humanizeError'

interface StartupItem {
  name: string
  command: string
  exe_path: string
  source: 'registry_user' | 'registry_machine' | 'folder_user' | 'folder_machine'
  requires_admin: boolean
  enabled: boolean
  icon: string | null
  reg_name?: string | null
  expand_sz?: boolean
}

const SOURCE_LABELS: Record<StartupItem['source'], string> = {
  registry_user: '用户注册表',
  registry_machine: '系统注册表',
  folder_user: '启动文件夹',
  folder_machine: '公共启动文件夹',
}

const items = ref<StartupItem[]>([])
const loading = ref(true)
const errorMsg = ref('')
const busyKey = ref('')
const msg = ref('')
/** 原始错误信息（复制按钮使用）；成功提示或首次操作前为空 */
const rawError = ref('')
const copied = ref(false)
let msgTimer: ReturnType<typeof setTimeout> | null = null
let copyTimer: ReturnType<typeof setTimeout> | null = null

function keyOf(item: StartupItem) {
  return `${item.source}:${item.name}`
}

function errText(e: unknown): string {
  return typeof e === 'string' ? e : e instanceof Error ? e.message : String(e ?? '')
}

async function load() {
  loading.value = true
  errorMsg.value = ''
  try {
    items.value = await invoke<StartupItem[]>('list_startup_items')
  } catch (e) {
    const raw = errText(e)
    rawError.value = raw
    errorMsg.value = `加载失败：${humanizeInvokeError(raw)}`
  }
  loading.value = false
}

async function toggle(item: StartupItem) {
  if (busyKey.value) return
  busyKey.value = keyOf(item)
  msg.value = ''
  const enabling = !item.enabled
  try {
    await invoke(enabling ? 'enable_startup_item' : 'disable_startup_item', { item })
    // 本地翻转状态：关闭的项保留在列表中，随时可再打开
    item.enabled = enabling
    msg.value = enabling
      ? `✓ 已重新打开「${item.name}」的开机自启动`
      : `✓ 已关闭「${item.name}」的开机自启动`
    rawError.value = ''
  } catch (e) {
    const raw = errText(e)
    rawError.value = raw
    msg.value = `✗ ${humanizeInvokeError(raw)}`
  }
  busyKey.value = ''
  if (msgTimer) clearTimeout(msgTimer)
  if (msg.value.startsWith('✓')) {
    // 成功提示短暂展示后自动消失；错误提示保持，直到用户关闭或进行下一次操作
    msgTimer = setTimeout(() => { msg.value = '' }, 4000)
  }
}

/** 一键复制原始错误信息到剪贴板（优先 Tauri 插件，失败回退 Web API） */
async function copyError() {
  if (!rawError.value) return
  try {
    await writeText(rawError.value)
  } catch {
    try {
      await navigator.clipboard.writeText(rawError.value)
    } catch {
      return
    }
  }
  copied.value = true
  if (copyTimer) clearTimeout(copyTimer)
  copyTimer = setTimeout(() => { copied.value = false }, 1500)
}

onMounted(load)
onUnmounted(() => {
  if (msgTimer) clearTimeout(msgTimer)
  if (copyTimer) clearTimeout(copyTimer)
})
</script>

<template>
  <div class="scrollbar-none relative flex h-full flex-col gap-6 overflow-y-auto px-1 py-1">
    <div v-if="loading" class="flex flex-1 items-center justify-center">
      <Loader2 class="h-4 w-4 animate-spin text-muted-foreground" />
    </div>

    <template v-else>
      <!-- ═══ 第三方启动项：边距分区，无背景块（与设置页同风格） ═══ -->
      <section class="space-y-2.5">
        <div class="flex items-center justify-between">
          <h3 class="text-[11px] font-semibold text-foreground/85">第三方启动项</h3>
          <Button
            size="icon-sm"
            variant="ghost"
            title="刷新"
            aria-label="刷新"
            @click="load"
          >
            <RefreshCw class="h-3.5 w-3.5" />
          </Button>
        </div>
        <p class="text-[10px] text-muted-foreground/70">
          开机时自动启动的非 Windows 应用，可在此关闭或重新打开（系统启动项已自动过滤）
        </p>

        <p v-if="errorMsg" class="flex items-center gap-1 py-1.5 text-[10px] text-destructive">
          <span class="min-w-0 flex-1 truncate" :title="errorMsg">{{ errorMsg }}</span>
          <button
            v-if="rawError"
            class="flex h-3.5 w-3.5 shrink-0 items-center justify-center rounded-sm transition-colors hover:bg-destructive/20"
            :title="copied ? '已复制' : '复制错误信息'"
            :aria-label="copied ? '已复制' : '复制错误信息'"
            @click="copyError"
          >
            <Check v-if="copied" class="h-2.5 w-2.5" />
            <Copy v-else class="h-2.5 w-2.5" />
          </button>
        </p>

        <!-- 空状态 -->
        <p v-if="!errorMsg && items.length === 0" class="py-1.5 text-[10px] text-muted-foreground/70">
          没有发现第三方开机自启动项
        </p>

        <!-- 列表 -->
        <div v-else-if="!errorMsg" class="space-y-0.5 pt-0.5">
          <div
            v-for="item in items"
            :key="keyOf(item)"
            class="group flex items-center gap-2 rounded px-1 py-1 transition-colors hover:bg-muted/20"
          >
            <!-- 应用微缩图标 -->
            <img
              v-if="item.icon"
              :src="item.icon"
              alt=""
              draggable="false"
              class="h-4 w-4 shrink-0 rounded-[3px]"
              :class="item.enabled ? '' : 'opacity-50'"
            />
            <AppWindow
              v-else
              class="h-4 w-4 shrink-0 text-muted-foreground/50"
              :class="item.enabled ? '' : 'opacity-50'"
            />

            <div class="min-w-0 flex-1">
              <p class="truncate text-[11px] text-foreground/90" :title="item.command || item.exe_path">
                {{ item.name }}
                <span
                  v-if="!item.enabled"
                  class="ml-1 rounded-full bg-white/10 px-1.5 py-px text-[9px] text-muted-foreground"
                >已关闭</span>
              </p>
              <p class="truncate text-[10px]" :class="item.enabled ? 'text-muted-foreground/60' : 'text-muted-foreground/40'">
                {{ item.exe_path || '—' }}
                <span class="text-muted-foreground/40">·</span>
                {{ SOURCE_LABELS[item.source] }}
                <span v-if="item.requires_admin" class="text-warning/80">· 需管理员</span>
              </p>
            </div>
            <Switch
              :checked="item.enabled"
              :disabled="busyKey !== ''"
              :title="item.enabled
                ? (item.requires_admin ? '关闭（系统级，需管理员权限）' : '关闭开机自启动')
                : '重新打开开机自启动'"
              :aria-label="`${item.enabled ? '关闭' : '打开'} ${item.name} 的开机自启动`"
              @update:checked="toggle(item)"
            />
          </div>
        </div>
      </section>

      <!-- 操作反馈 toast：悬浮于面板底部，滚动时始终可见。
           成功提示自动消失；错误提示提供「复制错误信息」与「关闭」，保持到用户处理完 -->
      <Transition name="toast-fade">
        <div
          v-if="msg"
          class="absolute bottom-2 left-1/2 z-20 flex max-w-[90%] -translate-x-1/2 items-center gap-1.5 rounded-md px-2.5 py-1.5 text-[11px] font-medium shadow-lg"
          :class="msg.startsWith('✓') ? 'bg-success/80 text-white' : 'bg-destructive/80 text-white'"
        >
          <span class="min-w-0 truncate" :title="msg">{{ msg }}</span>
          <template v-if="!msg.startsWith('✓') && rawError">
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
              @click="msg = ''"
            >
              <X class="h-3 w-3" />
            </button>
          </template>
        </div>
      </Transition>
    </template>
  </div>
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
