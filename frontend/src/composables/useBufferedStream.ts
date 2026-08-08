import { onScopeDispose, ref, shallowRef, type Ref, type ShallowRef } from 'vue'

/** 流缓冲器的节拍与背压配置。 */
export interface BufferedStreamOptions {
  /** 首个值发布前用于积累短突发数据的时间。 */
  initialBufferMs: number
  /** 后续值之间的最小展示间隔。 */
  intervalMs: number
  /** 队列允许保留的最大快照数，超出时丢弃最旧快照。 */
  maxQueueSize: number
}

/** `useBufferedStream` 返回的响应式状态与控制方法。 */
export interface BufferedStream<T> {
  value: ShallowRef<T>
  revision: Ref<number>
  push: (nextValue: T) => void
  reset: (nextValue: T) => void
}

/**
 * 将突发流快照转换为稳定的 UI 展示节拍，并用有界队列避免渲染长期落后。
 *
 * 该缓冲器面向“最新状态快照”，不是需要无损拼接的文本 token 流；队列满时
 * 会丢弃最旧快照，但始终保留最新状态。
 */
export function useBufferedStream<T>(
  initialValue: T,
  options: BufferedStreamOptions,
): BufferedStream<T> {
  const value = shallowRef(initialValue) as ShallowRef<T>
  const revision = ref(0)
  const queue: T[] = []
  let timer: ReturnType<typeof setTimeout> | null = null
  let hasPublished = false

  function clearTimer() {
    if (timer !== null) clearTimeout(timer)
    timer = null
  }

  function scheduleNext() {
    if (timer !== null || queue.length === 0) return
    const delay = hasPublished ? options.intervalMs : options.initialBufferMs
    timer = setTimeout(() => {
      timer = null
      const nextValue = queue.shift()
      if (nextValue === undefined) return

      value.value = nextValue
      revision.value += 1
      hasPublished = true
      scheduleNext()
    }, delay)
  }

  function push(nextValue: T) {
    queue.push(nextValue)
    if (queue.length > options.maxQueueSize) {
      queue.splice(0, queue.length - options.maxQueueSize)
    }
    scheduleNext()
  }

  function reset(nextValue: T) {
    clearTimer()
    queue.length = 0
    value.value = nextValue
    revision.value = 0
    hasPublished = false
  }

  onScopeDispose(clearTimer)

  return { value, revision, push, reset }
}
