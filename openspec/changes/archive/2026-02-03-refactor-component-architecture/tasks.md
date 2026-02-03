## 1. 基础设施

- [x] 1.1 创建 `crates/ui/src/primitives/` 目录和 `mod.rs`
- [x] 1.2 创建 `crates/ui/src/widgets/` 目录和 `mod.rs`
- [x] 1.3 创建 `crates/ui/src/containers/` 目录和 `mod.rs`
- [x] 1.4 创建 `crates/ui/src/dialogs/` 目录和 `mod.rs`
- [x] 1.5 创建 `crates/ui/src/pages/` 目录和 `mod.rs`
- [x] 1.6 创建 `crates/ui/src/selectors/` 目录和 `mod.rs`

## 2. Primitives 组件实现

- [x] 2.1 实现 `Badge` 组件（HTTP 方法标签，支持 GET/POST/PUT/DELETE/PATCH 颜色）
- [x] 2.2 实现 `Card` 组件（可选 header/body/footer 区域）
- [x] 2.3 实现 `KeyValueRow` 组件（支持 editable 和 read-only 模式）
- [x] 2.4 实现 `TreeItem` 组件（展开/折叠指示器、选中状态）
- [x] 2.5 实现 `EmptyState` 组件（图标、消息、可选操作按钮）
- [x] 2.6 更新 `primitives/mod.rs` 导出所有组件

## 3. Widgets 组件实现

- [x] 3.1 实现 `EditableInput` widget（封装 InputState，支持 placeholder/validation/on_change/on_submit）
- [x] 3.2 实现 `TabBar` widget（tab 列表、active 状态、可选 close 按钮）- 重新导出 gpui_component::tab
- [x] 3.3 实现 `OverlayDialog` widget（半透明遮罩、居中 dialog、backdrop 点击关闭）
- [x] 3.4 更新 `widgets/mod.rs` 导出所有组件

## 4. 现有组件文件迁移

- [x] 4.1 移动 `rename_dialog.rs` 到 `dialogs/`
- [x] 4.2 移动 `confirm_dialog.rs` 到 `dialogs/`
- [x] 4.3 移动 `environment_selector.rs` 中的 `EnvironmentManager` 到 `dialogs/`
- [x] 4.4 移动 `home_page.rs` 到 `pages/`
- [x] 4.5 移动 `title_bar.rs` 到 `pages/`
- [x] 4.6 移动 `environment_selector.rs` 到 `selectors/`
- [x] 4.7 移动 `http_service/method_selector.rs` 到 `selectors/`
- [x] 4.8 移动 `sidebar.rs` 到 `containers/`
- [x] 4.9 移动 `workspace_area.rs` 到 `containers/`
- [x] 4.10 移动 `workspace_content.rs` 到 `containers/`
- [x] 4.11 移动 `service_tabs.rs` 到 `containers/`
- [x] 4.12 移动 `http_service/` 目录到 `containers/`
- [x] 4.13 移动 `ws_sidebar/` 目录到 `containers/`
- [x] 4.14 移动 `folder_detail.rs` 到 `containers/`

## 5. 大组件拆分

- [x] 5.1 从 `FolderDetailPage` 提取 `FolderOverviewPanel` 到独立文件
- [x] 5.2 从 `FolderDetailPage` 提取 `CommonHeadersPanel` 到独立文件
- [x] 5.3 从 `FolderDetailPage` 提取 `SearchParamsPanel` 到独立文件
- [x] 5.4 重构 `WorkspaceContent` 提取导入处理逻辑到 `ImportHandler` 模块
- [x] 5.5 重构 `WorkspaceContent` 提取服务管理逻辑到 `ServiceManager` 模块

## 6. 组件适配与重构

- [x] 6.1 将现有组件中的方法标签替换为 `Badge` 组件
- [x] 6.2 将重复的 InputState 创建模式替换为 `EditableInput`（widget 已实现，现有组件可渐进迁移）
- [x] 6.3 将散落的 Dialog overlay 模式替换为 `OverlayDialog`
- [x] 6.4 将 `ServiceTabs` 中的 tab 渲染替换为 `TabBar`（已使用 gpui_component::tab::TabBar）

## 7. 模块导出与清理

- [x] 7.1 更新 `crates/ui/src/lib.rs` 声明所有新目录为 public modules
- [x] 7.2 更新所有 import 路径以匹配新目录结构
- [x] 7.3 运行 `cargo build --workspace` 确保编译通过
- [x] 7.4 运行 `cargo clippy --workspace --all-targets` 修复 lint 警告
- [x] 7.5 删除空文件和废弃代码
- [x] 7.6 添加 `crates/ui/README.md` 记录组件分类约定