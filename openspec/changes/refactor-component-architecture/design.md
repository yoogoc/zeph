## Context

当前 `crates/ui/src/` 采用扁平结构，所有组件混合在一起：
- 有状态容器（如 `WorkspaceContent`、`HttpService`）与无状态展示组件混杂
- 部分大组件内嵌了多个子组件定义（如 `FolderDetailPage` 内含 3 个 Panel）
- 重复的 UI 模式（Dialog overlay、InputState 创建）散落在各处
- 缺乏统一的命名约定，难以快速判断组件职责

技术栈：Rust + GPUI 框架，组件使用 `Entity<T>` 管理状态，通过 `EventEmitter` 进行父子通信。

## Goals / Non-Goals

**Goals:**
- 建立清晰的组件分层结构（primitives → widgets → containers → pages）
- 提取可复用的无状态基础组件，减少重复代码
- 统一状态管理模式和命名约定
- 降低大组件复杂度，提升可维护性

**Non-Goals:**
- 不改变现有业务逻辑和功能
- 不引入新的状态管理库或框架
- 不重构 `crates/models`、`crates/workspace` 等非 UI 层代码
- 不处理性能优化（这是后续工作）

## Decisions

### 1. 目录分层策略

**决定**：按状态复杂度和复用性划分为 6 个目录层级

| 目录 | 职责 | 状态特征 |
|------|------|---------|
| `primitives/` | 纯渲染基础元素 | 无状态，只接收 props |
| `widgets/` | 可复用交互组件 | 轻量状态，通用功能 |
| `containers/` | 业务容器组件 | 持有 Entity/Subscription |
| `dialogs/` | 模态对话框 | 输入验证、确认流程 |
| `pages/` | 路由级页面 | 组合多个 Container |
| `selectors/` | 选择器组件 | 下拉、picker 类交互 |

**替代方案**：
- 仅分 `stateless/` 和 `stateful/` 两层 → 粒度太粗，widgets 和 containers 职责不同
- 按功能域划分（如 `http/`、`workspace/`）→ 已有此模式，但缺乏状态分类

**理由**：6 层结构平衡了分类粒度和实用性，与 React/Vue 社区最佳实践一致。

### 2. Primitives 实现方式

**决定**：使用独立 struct 实现，而非纯函数

```rust
// 推荐：struct 形式
pub struct Badge {
    label: SharedString,
    color: Hsla,
}

impl RenderOnce for Badge {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement { ... }
}
```

**替代方案**：
- 纯函数 `fn badge(label, color) -> impl IntoElement` → 缺少类型约束，参数多时可读性差

**理由**：struct 形式支持 builder pattern，类型安全，与 GPUI 现有模式一致。

### 3. InputState 封装策略

**决定**：创建 `EditableInput` widget 封装常见模式

```rust
pub struct EditableInput {
    state: Entity<InputState>,
    on_change: Option<Box<dyn Fn(&str, &mut Window, &mut Context<Self>)>>,
    on_submit: Option<Box<dyn Fn(&str, &mut Window, &mut Context<Self>)>>,
}
```

**替代方案**：
- 继续在每个组件中直接创建 `InputState` → 重复代码多，行为不一致
- 使用宏简化创建 → 过度抽象，调试困难

**理由**：Widget 封装保持了灵活性，同时统一了 placeholder、validation、focus 等通用行为。

### 4. Dialog Overlay 统一方案

**决定**：创建 `OverlayDialog` 容器组件

```rust
pub struct OverlayDialog<D: RenderOnce> {
    dialog: Option<D>,
    on_dismiss: Option<Box<dyn Fn(&mut Window, &mut Context<Self>)>>,
}
```

**替代方案**：
- 每个组件自行实现 overlay → 当前状态，代码重复
- 全局 DialogManager → 过于复杂，对简单场景是过度设计

**理由**：容器组件模式简单直接，不引入全局状态，适合当前规模。

### 5. 大组件拆分策略

**决定**：按职责边界拆分，保持原有 Entity 通信模式

- `WorkspaceContent` → 提取 `ImportHandler`、`ServiceManager`、`FolderManager` 等子模块
- `FolderDetailPage` → 将 3 个 Panel 移到独立文件

**理由**：渐进式拆分，最小化对现有事件流的影响。

## Risks / Trade-offs

| 风险 | 影响 | 缓解措施 |
|------|------|---------|
| 大规模文件移动导致 merge 冲突 | 中 | 分阶段实施，每阶段完成后立即合并 |
| 新目录结构学习成本 | 低 | 在 `crates/ui/README.md` 中记录约定 |
| 拆分后组件间通信变复杂 | 中 | 保持 EventEmitter 模式，仅改变文件位置 |
| 过度抽象 primitives | 低 | 仅抽取已重复 3 次以上的模式 |

## Migration Plan

1. **Phase 1 - 基础设施**：创建目录结构，添加 `primitives/` 和 `widgets/`
2. **Phase 2 - Primitives 提取**：从现有组件提取 5 个基础组件
3. **Phase 3 - Widgets 创建**：实现 `EditableInput`、`TabBar`、`OverlayDialog`
4. **Phase 4 - 文件重组**：按新结构移动现有组件文件
5. **Phase 5 - 大组件拆分**：处理 `WorkspaceContent` 和 `FolderDetailPage`
6. **Phase 6 - 清理**：更新 `lib.rs` 导出，删除废弃代码

每阶段完成后运行 `cargo build --workspace && cargo clippy` 确保构建正常。

## Open Questions

- [ ] 是否需要为 primitives 添加 Storybook 风格的预览页面？
- [ ] `selectors/` 是否应该合并到 `widgets/`？