# Implementation Plan: Service and Folder Rename and Save

## Overview

本实现计划将 Service 和 Folder 的重命名和保存功能分解为离散的编码步骤。每个任务都引用具体的需求，并包含属性测试作为子任务以确保正确性。

## Tasks

- [x] 1. 创建 RenameDialog 通用组件
  - 创建 `crates/ui/src/rename_dialog.rs` 文件
  - 实现 RenameDialog 结构体，包含输入框、验证逻辑和事件发射
  - 实现名称验证方法（空白检查、长度限制、非法字符）
  - 实现 Render trait，显示对话框 UI
  - 在 `crates/ui/src/lib.rs` 中导出新模块
  - _Requirements: 1.2, 2.2, 6.1, 6.2_

- [ ]* 1.1 为 RenameDialog 编写属性测试
  - **Property 1: 空白名称验证**
  - **Validates: Requirements 1.5, 2.5, 4.5, 6.1**

- [x] 2. 扩展 WorkspaceSidebarEvent 支持重命名
  - 在 `crates/ui/src/ws_sidebar/mod.rs` 中添加 `RenameService(Uuid)` 和 `RenameFolder(Uuid)` 事件变体
  - 在 WorkspaceSidebar 中实现事件处理逻辑
  - 添加显示 RenameDialog 的方法
  - _Requirements: 1.1, 1.2, 2.1, 2.2_

- [x] 3. 更新 PopupMenu 添加 Rename 选项
  - 修改 `crates/ui/src/ws_sidebar/popup_menu.rs`
  - 为 Service 和 Folder 添加 "Rename" 菜单项
  - 连接菜单项到 RenameService/RenameFolder 事件
  - _Requirements: 1.1, 2.1_

- [x] 4. 实现 Service 重命名数据库更新
  - 修改 `crates/storage/src/service.rs` 中的 ServiceStore::update 方法（如果需要）
  - 确保 update 方法正确更新 name 字段和 updated_at 时间戳
  - 添加名称验证逻辑到存储层
  - _Requirements: 1.3, 6.1, 6.2_

- [ ]* 4.1 为 Service 重命名编写属性测试
  - **Property 2: Service 名称数据库更新**
  - **Validates: Requirements 1.3, 1.7**

- [x] 5. 实现 FolderStore::update 通用方法
  - 在 `crates/storage/src/folder.rs` 中添加 `update` 方法
  - 方法签名：`pub fn update(&self, folder_id: Uuid, name: Option<&str>, description: Option<Option<&str>>, parent_id: Option<Option<Uuid>>) -> Result<Folder>`
  - 实现数据库 UPDATE 语句
  - 添加名称和描述验证逻辑
  - 返回更新后的 Folder 对象
  - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5_

- [ ]* 5.1 为 FolderStore::update 编写属性测试
  - **Property 3: Folder 名称数据库更新**
  - **Property 10: FolderStore 更新方法**
  - **Property 11: FolderStore 更新错误处理**
  - **Validates: Requirements 2.3, 4.3, 5.2, 5.3, 5.4**

- [x] 6. 实现 Folder 重命名数据库更新
  - 在 WorkspaceSidebar 的重命名事件处理中调用 FolderStore::update
  - 处理成功和失败情况
  - _Requirements: 2.3_

- [x] 7. 实现 WorkspaceRegistry 通知机制
  - 在 Service/Folder 更新后通过 WorkspaceRegistry 通知所有订阅者
  - 确保 Sidebar、ServiceTabs 和详情页都能接收到更新通知
  - 实现 UI 组件的刷新逻辑
  - _Requirements: 1.4, 2.4, 8.1, 8.2_

- [ ]* 7.1 为 UI 同步编写属性测试
  - **Property 4: 数据更新后 UI 同步**
  - **Validates: Requirements 1.4, 2.4, 4.4, 8.1, 8.2**

- [x] 8. Checkpoint - 确保重命名功能正常工作
  - 测试从 Sidebar 右键菜单重命名 Service 和 Folder
  - 验证数据库更新正确
  - 验证 UI 同步更新
  - 确保所有测试通过，如有问题请询问用户

- [x] 9. 为 ServiceTabs 添加 Tab 标题编辑功能
  - 修改 `crates/ui/src/service_tabs.rs`
  - 添加 `editing_tab: Option<ContentType>` 和 `edit_input: Option<Entity<InputState>>` 字段
  - 实现点击已聚焦 Tab 进入编辑模式的逻辑
  - 实现 Enter 键保存、Escape 键取消的键盘处理
  - 添加 `Rename(ContentType, SharedString)` 事件变体
  - _Requirements: 1.6, 1.7, 1.8_

- [ ]* 9.1 为 Tab 编辑取消编写属性测试
  - **Property 5: Tab 编辑取消恢复**
  - **Validates: Requirements 1.8**

- [x] 10. 连接 ServiceTabs 重命名事件到数据库
  - 在 WorkspaceArea 或相关组件中订阅 ServiceTabsEvent::Rename
  - 调用 ServiceStore::update 更新数据库
  - 通过 WorkspaceRegistry 通知更新
  - _Requirements: 1.7_

- [x] 11. 为 HttpService 添加保存状态管理
  - 修改 `crates/ui/src/http_service/http_service.rs`
  - 添加 `is_modified: bool`、`original_config: HttpConfig`、`service_id: Uuid` 字段
  - 实现 `mark_modified()` 方法，在任何配置修改时调用
  - 实现 `has_unsaved_changes()` 方法检查是否有未保存修改
  - 订阅所有输入组件的变化事件
  - _Requirements: 3.1, 3.2_

- [ ]* 11.1 为 Service 配置修改标记编写属性测试
  - **Property 6: Service 配置修改标记**
  - **Validates: Requirements 3.1**

- [x] 12. 实现 HttpService 保存按钮和保存逻辑
  - 在 HttpService UI 中添加保存按钮（仅在 is_modified 为 true 时显示）
  - 实现 `save_changes()` 方法
  - 实现 `collect_current_config()` 方法收集当前所有配置
  - 调用 ServiceStore::update 保存到数据库
  - 处理保存成功和失败情况
  - 保存成功后清除 is_modified 标记
  - _Requirements: 3.2, 3.3, 3.4, 3.6_

- [ ]* 12.1 为 Service 配置保存编写属性测试
  - **Property 7: Service 配置保存往返**
  - **Property 8: 保存按钮状态管理**
  - **Property 9: 保存失败状态保持**
  - **Validates: Requirements 3.3, 3.2, 3.4, 3.6, 8.3, 8.4**

- [x] 13. 实现关闭未保存 Service 的确认对话框
  - 在 ServiceTabs 的 Close 事件处理中检查是否有未保存修改
  - 如果有未保存修改，显示确认对话框
  - 提供"保存"、"不保存"、"取消"三个选项
  - _Requirements: 3.5_

- [x] 14. Checkpoint - 确保 Service 保存功能正常工作
  - 测试修改 Service 配置后保存
  - 验证保存按钮状态变化
  - 验证关闭未保存 Service 的提示
  - 确保所有测试通过，如有问题请询问用户

- [x] 15. 为 FolderOverviewPanel 添加编辑模式
  - 修改 `crates/ui/src/folder_detail.rs` 中的 FolderOverviewPanel
  - 添加 `is_editing: bool`、`name_input: Option<Entity<InputState>>`、`description_input: Option<Entity<InputState>>` 字段
  - 实现 `enter_edit_mode()` 方法，将只读字段替换为输入框
  - 实现 `exit_edit_mode()` 方法，恢复只读显示
  - 添加"编辑"按钮到 Overview 面板
  - _Requirements: 4.1_

- [x] 16. 实现 FolderOverviewPanel 保存逻辑
  - 实现 `save_changes()` 方法
  - 实现 `validate_inputs()` 方法验证名称和描述
  - 添加保存按钮（仅在编辑模式下显示）
  - 发射 FolderDetailPageEvent 通知父组件保存
  - 处理保存成功和失败情况
  - _Requirements: 4.2, 4.3, 4.4, 4.5_

- [ ]* 16.1 为 Folder 详情编辑编写属性测试
  - **Property 12: 验证错误显示**
  - **Validates: Requirements 4.3, 4.5, 6.4**

- [x] 17. 连接 FolderDetailPage 保存事件到数据库
  - 在 FolderDetailPage 中订阅 FolderOverviewPanel 的保存事件
  - 调用 FolderStore::update 更新数据库
  - 通过 WorkspaceRegistry 通知更新
  - 更新 FolderDetailPage 显示的 folder 数据
  - _Requirements: 4.3, 4.4_

- [x] 18. 添加国际化文本
  - 在 `locales/zh-CN.yml` 中添加所有新增的中文翻译
  - 在 `locales/en.yml` 中添加所有新增的英文翻译
  - 包括：重命名对话框、保存按钮、错误信息、确认对话框等
  - _Requirements: 7.1, 7.2_

- [x] 19. 添加输入验证和错误显示
  - 在所有输入组件中添加实时验证
  - 在输入框附近显示具体的验证错误信息
  - 使用红色边框标记无效输入
  - 确保错误信息清晰且可操作
  - _Requirements: 6.1, 6.2, 6.3, 6.4_

- [x] 20. 实现边界情况处理
  - 处理名称长度超过 255 字符的情况
  - 处理描述长度超过 1000 字符的情况
  - 添加字符计数器显示剩余字符数
  - _Requirements: 6.2, 6.3_

- [x] 21. Final Checkpoint - 完整功能测试
  - 测试所有重命名入口（Sidebar、Tab、详情页）
  - 测试所有保存功能（Service、Folder）
  - 测试所有验证规则
  - 测试 UI 同步和状态管理
  - 测试国际化文本切换
  - 运行所有单元测试和属性测试
  - 确保所有测试通过，如有问题请询问用户

## Notes

- 标记为 `*` 的任务是可选的测试任务，可以跳过以加快 MVP 开发
- 每个任务都引用了具体的需求编号以便追溯
- Checkpoint 任务确保增量验证
- 属性测试验证通用正确性属性
- 单元测试验证具体示例和边界情况
