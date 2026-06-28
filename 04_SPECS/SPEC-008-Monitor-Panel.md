# SPEC-008: Vue 监控面板

## 1. 目标

实现进程监控面板全部 UI，功能等价于旧版（egui 版本）。

## 2. 数据流

```typescript
// frontend/src/composables/useMonitor.ts
import { invoke } from '@tauri-apps/api/core';
import { ref, shallowRef, onMounted, onUnmounted } from 'vue';

interface Snapshot {
  summary: SystemSummary;
  processes: ProcessInfo[];
}
interface SystemSummary {
  cpu_total: number;
  mem_used_mb: number;
  mem_total_mb: number;
  process_count: number;
}
interface ProcessInfo {
  pid: number;
  name: string;
  cpu: number;
  mem_mb: number;
  status: string;
}
```

轮询逻辑（带生命周期管理）：
```typescript
export function useMonitor() {
  const processes = shallowRef<ProcessInfo[]>([]);
  const summary = ref<SystemSummary | null>(null);
  const loading = ref(true);
  const error = ref<string | null>(null);
  let intervalId: ReturnType<typeof setInterval> | null = null;

  async function fetchProcesses() {
    try {
      const snap = await invoke<Snapshot>('get_processes');
      summary.value = snap.summary;
      processes.value = snap.processes;
      loading.value = false;
      error.value = null;
    } catch (e) {
      error.value = String(e);
      loading.value = false;
    }
  }

  onMounted(() => {
    fetchProcesses();
    intervalId = setInterval(fetchProcesses, 2000);
  });

  onUnmounted(() => {
    if (intervalId) clearInterval(intervalId);
  });

  return { processes, summary, loading, error, fetchProcesses };
}
```

## 3. 组件结构

```
MonitorPanel.vue
├── SummaryBar.vue          (紧凑摘要行)
├── SearchInput.vue         (搜索框)
└── ProcessTable.vue        (进程表格, shadcn-vue Table)
```

### 3.1 SummaryBar.vue

```
CPU: 312%  |  内存: 6.2/16.0GB  |  进程: 142
```

- CPU 颜色类: `<50% text-primary`, `50-80% text-amber-400`, `>80% text-red-400`
- 内存颜色类: `<65% text-teal-400`, `65-85% text-amber-400`, `>85% text-red-400`
- `<Separator />` 分隔

### 3.2 ProcessTable.vue

shadcn-vue `<Table>` + `<TableBody>` + `<TableRow>` 实现：

```vue
<template>
  <Table>
    <TableHeader>
      <TableRow>
        <TableHead class="cursor-pointer" @click="sort('name')">
          Name{{ sortIcon('name') }}
        </TableHead>
        <TableHead class="cursor-pointer text-right" @click="sort('cpu')">
          CPU%{{ sortIcon('cpu') }}
        </TableHead>
        <TableHead class="cursor-pointer text-right" @click="sort('mem')">
          Mem{{ sortIcon('mem') }}
        </TableHead>
        <TableHead class="text-right">Mem%</TableHead>
        <TableHead class="w-10"></TableHead>
      </TableRow>
    </TableHeader>
    <TableBody>
      <TableRow v-for="p in displayed" :key="p.pid">
        <TableCell class="font-medium truncate max-w-[140px]">{{ p.name }}</TableCell>
        <TableCell class="text-right" :class="cpuColorClass(p.cpu)">
          {{ p.cpu.toFixed(1) }}
        </TableCell>
        <TableCell class="text-right">{{ formatMem(p.mem_mb) }}</TableCell>
        <TableCell class="text-right text-muted-foreground">
          {{ memPercent(p.mem_mb) }}
        </TableCell>
        <TableCell class="text-center">
          <Button variant="ghost" size="icon" @click="confirmKill(p)">×</Button>
        </TableCell>
      </TableRow>
      <!-- 空状态 -->
      <TableRow v-if="displayed.length === 0 && !loading">
        <TableCell colspan="5" class="text-center text-muted-foreground">
          {{ searchQuery ? `没有进程匹配 "${searchQuery}"` : '等待数据...' }}
        </TableCell>
      </TableRow>
    </TableBody>
  </Table>
</template>
```

### 3.3 过滤逻辑

```typescript
const displayed = computed(() => {
  if (!summary.value) return [];
  const all = processes.value;
  // 有搜索词: 不过滤阈值, 纯 name match
  if (searchQuery.value) {
    const q = searchQuery.value.toLowerCase();
    return all.filter(p => p.name.toLowerCase().includes(q));
  }
  // 无搜索词: 仅显示 CPU>10% 或 MEM>200MB
  return all.filter(p => p.cpu > 10 || p.mem_mb > 200);
});
```

### 3.4 排序

```typescript
const sortField = ref<'name' | 'cpu' | 'mem'>('cpu');
const sortAsc = ref(false);

const sorted = computed(() => {
  const arr = [...displayed.value];
  arr.sort((a, b) => {
    let cmp = 0;
    if (sortField.value === 'name') cmp = a.name.localeCompare(b.name);
    else if (sortField.value === 'cpu') cmp = b.cpu - a.cpu;
    else cmp = b.mem_mb - a.mem_mb;
    return sortAsc.value ? -cmp : cmp;
  });
  return arr;
});
```

### 3.5 Kill 确认 (shadcn-vue AlertDialog)

```vue
<AlertDialog v-model:open="showKillDialog">
  <AlertDialogContent>
    <AlertDialogHeader>
      <AlertDialogTitle>终止进程</AlertDialogTitle>
      <AlertDialogDescription>
        确定要终止 {{ killTarget?.name }} (PID: {{ killTarget?.pid }}) 吗？
      </AlertDialogDescription>
    </AlertDialogHeader>
    <AlertDialogFooter>
      <AlertDialogCancel>取消</AlertDialogCancel>
      <AlertDialogAction @click="executeKill">确认终止</AlertDialogAction>
    </AlertDialogFooter>
  </AlertDialogContent>
</AlertDialog>
```

Kill 结果反馈使用 `<Alert variant="destructive">` + 3s 后自动消失（`setTimeout`）。

## 4. 列规格

| 列 | 宽度 | 对齐 | 备注 |
|---|---|---|---|
| Name | max-w-[140px] truncate | left | text-ellipsis |
| CPU% | auto | right | 着色: 80% 红, 50% 琥珀, 其他正常 |
| Mem | auto | right | 自动 GB/MB 格式化 |
| Mem% | auto | right | text-muted-foreground |
| × | w-10 | center | variant=ghost size=icon |

## 5. 状态处理

| 状态 | UI |
|---|---|
| loading=true (首次) | `<Skeleton class="h-8" />` 占位行 |
| error !== null | `<Alert variant="destructive">` + 重试按钮 |
| 搜索无结果 | TableRow empty state "没有进程匹配" |
| 监控断开 | invoke 返回 Err → error 状态 |
