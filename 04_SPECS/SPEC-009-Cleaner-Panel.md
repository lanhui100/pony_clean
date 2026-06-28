# SPEC-009: Vue 清理面板

## 1. 目标

实现 C盘安全清理面板全部 UI，功能等价于旧版（egui 版本）+ 删除确认弹窗 + 结果反馈。

## 2. 数据流

```typescript
// frontend/src/composables/useCleaner.ts
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { ref, onMounted, onUnmounted } from 'vue';

interface ScanProgressPayload {
  scanned: number;
  current: string;
}
interface ScanDonePayload {
  total_items: number;
  total_bytes: number;
}
interface CleanItem {
  path: string;
  size_bytes: number;
  category: string;
}
```

事件监听（注意 onUnmounted 清理）：
```typescript
export function useCleaner() {
  const state = ref<'idle'|'scanning'|'done'|'cancelled'|'error'|'deleting'>('idle');
  const scanned = ref(0);
  const currentFile = ref('');
  const items = ref<CleanItem[]>([]);
  const totalBytes = ref(0);
  let unlistenProgress: UnlistenFn | null = null;
  let unlistenDone: UnlistenFn | null = null;

  onMounted(async () => {
    unlistenProgress = await listen<ScanProgressPayload>('scan-progress', (e) => {
      scanned.value = e.payload.scanned;
      currentFile.value = e.payload.current;
    });
    unlistenDone = await listen<ScanDonePayload>('scan-done', (e) => {
      state.value = 'done';
      totalBytes.value = e.payload.total_bytes;
    });
  });

  onUnmounted(() => {
    unlistenProgress?.();
    unlistenDone?.();
  });

  async function startScan() { ... }
  function cancelScan() { ... }
  async function executeDelete(paths: string[]) { ... }

  return { state, scanned, currentFile, items, totalBytes, startScan, cancelScan, executeDelete };
}
```

## 3. 状态机

```
idle → scanning → done → (confirm) → deleting → idle
                 → cancelled
                 → error
```

## 4. 组件结构

```
CleanerPanel.vue
├── [idle] CleanIdle.vue         (居中扫描按钮)
├── [scanning] ScanProgress.vue  (进度 + 文件数)
├── [done] CleanResult.vue       (摘要 + 分类 + 操作栏)
│   ├── CategoryLegend.vue       (分类图例)
│   └── CleanCategory.vue        (单分类折叠 + checkbox)
├── CleanConfirmDialog.vue       (删除确认弹窗)
└── CleanResultToast.vue         (删除结果反馈)
```

## 5. 各状态 UI

### 5.1 Idle
居中 "C盘安全清理" 标题 + 描述 + 蓝色 `<Button>` "开始扫描"。

### 5.2 Scanning
`<Progress :value="null" />` 不确定进度 + "已扫描 N 个文件" + 当前路径 + "取消" 按钮。

### 5.3 Done — 分类聚合

**分类数据来源**: 前端从 scan-progress 期间收到的 `ItemsFound` 事件中聚合（需新增 `items-found` Event），或后端在 `scan-done` 中附带分类汇总。推荐方案：后端在 `scan-done` payload 中附带 `categories: Vec<CategorySummary>`。

```typescript
interface CategorySummary {
  category: string;
  total_bytes: number;
  items: CleanItem[];
}
interface ScanDonePayload {
  total_items: number;
  total_bytes: number;
  categories: CategorySummary[];
}
```

### 5.4 分类折叠 (CleanCategory.vue)

**重要: checkbox 使用 `:checked` + `@update:checked` 模式，避免 `v-model` 与事件冲突**:

```vue
<Collapsible v-for="cat in categories" v-model:open="cat.open">
  <CollapsibleTrigger class="flex items-center gap-2 w-full">
    <Checkbox
      :checked="cat.allChecked"
      @update:checked="(v) => toggleCategory(cat, v)"
    />
    <span class="font-medium">{{ cat.label }} ({{ formatSize(cat.totalBytes) }})</span>
    <ChevronDownIcon class="ml-auto" />
  </CollapsibleTrigger>
  <CollapsibleContent>
    <div v-for="item in cat.items" class="flex items-center gap-2 pl-8 py-1">
      <Checkbox
        :checked="item.checked"
        @update:checked="(v) => toggleItem(cat, item, v)"
      />
      <span class="text-sm">{{ formatSize(item.size_bytes) }}</span>
      <span class="text-xs text-muted-foreground truncate">{{ item.path }}</span>
    </div>
  </CollapsibleContent>
</Collapsible>
```

`toggleCategory` 函数统一更新该分类下所有项的 `checked` 状态，避免 `v-model` 双向绑定导致的重复更新。

### 5.5 分类图例

使用 `<span class="w-2.5 h-2.5 rounded-full bg-blue-400" />` 替代 emoji 方块（避免 WebView2 表情符号渲染不一致）。

### 5.6 底部操作栏

```html
<div class="flex items-center justify-between pt-2 border-t">
  <span class="text-sm text-muted-foreground">
    已选 {{ checkedCount }}/{{ totalCount }} 项
  </span>
  <div class="flex gap-2">
    <Button variant="outline" size="sm" @click="toggleSelectAll">
      {{ allSelected ? '取消全选' : '全选' }}
    </Button>
    <Button variant="destructive" size="sm" :disabled="checkedCount === 0" @click="showConfirm = true">
      清理选中
    </Button>
  </div>
</div>
```

### 5.7 删除确认

shadcn-vue `<AlertDialog>`，提示"即将永久删除 N 个文件，此操作不可撤销"。

### 5.8 删除结果

shadcn-vue `<Alert>` + 3s 自动消失（`setTimeout`）。失败项可展开错误列表。

## 6. 空结果

`totalBytes === 0` → 显示 ✓ + "没有发现可清理文件" + "你的 C 盘状况良好"。

## 7. 分类颜色

| 分类 | Tailwind Class |
|---|---|
| temp | `bg-blue-400` |
| cache | `bg-purple-400` |
| prefetch | `bg-green-400` |
| recycle_bin | `bg-amber-400` |

## 8. 性能

- 单分类 >500 项时: 默认仅渲染前 50 项 + "展开全部" 链接
- 使用 `computed` + `key` 优化 `v-for`
- 删除操作在 `spawn_blocking` 中执行，不阻塞 UI
