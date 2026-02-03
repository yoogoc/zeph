## Why

当前 `crates/ui` 中的组件缺乏统一的状态分类约定，导致：
- 无法快速判断组件是否可复用（命名无规律）
- 大组件职责过重（`WorkspaceContent` 954 行，`FolderDetailPage` 内嵌 3 个面板约 1500 行）
- 展示逻辑与业务状态管理混合，增加维护成本
- 重复模式散落各处（Dialog overlay、InputState 创建）

## What Changes

- **新增目录分层**：按状态特征将组件分类到 `primitives/`、`widgets/`、`containers/`、`dialogs/`、`pages/`、`selectors/`
- **提取无状态基础组件**：从现有组件中抽取纯渲染逻辑（卡片、标签、键值对行、树节点、空状态）
- **统一输入组件封装**：创建 `EditableInput` widget 封装 `InputState` 的重复创建模式
- **拆分大组件**：将 `WorkspaceContent` 和 `FolderDetailPage` 拆分为职责单一的子模块
- **统一 Dialog 模式**：创建 `OverlayDialog` 容器替换散落的 overlay 实现
- **建立命名约定**：Primitives 用名词或 `*View`，Containers 用业务名或 `*Container`，Dialogs 用 `*Dialog`

## Capabilities

### New Capabilities

- `ui-primitives`: 无状态基础 UI 组件层（Card、Badge、KeyValueRow、TreeItem、EmptyState）
- `ui-widgets`: 轻量状态可复用组件层（EditableInput、TabBar、OverlayDialog）
- `component-organization`: 组件目录结构规范与命名约定

### Modified Capabilities

无现有 specs 需要修改。

## Impact

- **代码变更**：`crates/ui/src/` 目录结构重组，现有组件文件移动或拆分
- **模块导出**：`crates/ui/src/lib.rs` 需更新导出路径
- **依赖关系**：现有组件 import 路径变更，需批量更新
- **测试**：如有 UI 测试需同步更新路径
- **构建**：无破坏性变更，重构后 `cargo build` 应正常通过