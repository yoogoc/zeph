# Implementation Plan: OpenAPI 导入功能重构

## Overview

重构 OpenAPI 导入功能，简化事件处理，支持从文件夹菜单触发导入，并支持 URL 导入。实现将分为：创建简化的导入对话框、扩展菜单、集成事件处理、添加 URL 支持。

## Tasks

- [-] 1. 创建简化的导入对话框组件
  - [x] 1.1 创建 `SimpleImportDialog` 结构体和基本 UI
    - 包含内容输入区域、目标文件夹显示、创建子文件夹选项
    - 只发出 `Import` 和 `Cancel` 两种事件
    - _Requirements: 2.1, 2.2, 2.3, 2.5, 3.1, 3.2, 3.3_
  - [x] 1.2 实现服务数量预览显示
    - 解析输入内容并显示将导入的服务数量
    - _Requirements: 2.4_
  - [x] 1.3 实现错误显示区域
    - 显示解析错误、验证错误等
    - _Requirements: 5.1, 5.3, 5.4_

- [x] 2. 添加 URL 导入支持
  - [x] 2.1 实现 URL 检测函数
    - 检测输入是否以 http:// 或 https:// 开头
    - _Requirements: 6.1_
  - [ ]* 2.2 为 URL 检测编写属性测试
    - **Property 1: URL 检测正确性**
    - **Validates: Requirements 6.1**
  - [x] 2.3 实现 URL 内容下载
    - 使用异步请求下载 OpenAPI 内容
    - 显示加载状态
    - _Requirements: 6.2, 6.3, 6.5_
  - [x] 2.4 实现网络错误处理
    - 处理连接失败、超时、非 200 响应等
    - _Requirements: 6.4_

- [x] 3. 扩展 Popup Menu
  - [x] 3.1 在服务文件夹菜单中添加"导入 OpenAPI"选项
    - 点击后打开对话框并预设目标文件夹
    - _Requirements: 1.1, 1.2_
  - [x] 3.2 在服务根节点菜单中添加"导入 OpenAPI"选项
    - 点击后打开对话框，目标文件夹为空
    - _Requirements: 1.3, 1.4_

- [-] 4. 集成事件处理
  - [ ] 4.1 在 WorkspaceSidebar 中添加新事件类型
    - `ShowImportDialog(Option<Uuid>)` - 显示导入对话框
    - `ExecuteImport(SimpleImportConfig)` - 执行导入
    - _Requirements: 3.4, 3.5_
  - [ ] 4.2 在 WorkspaceContent 中处理导入事件
    - 调用 WorkspaceRegistry 执行导入
    - 处理导入结果和错误
    - _Requirements: 4.4, 4.5_

- [x] 5. Checkpoint - 确保基本功能正常
  - 确保所有测试通过，如有问题请询问用户

- [-] 6. 实现导入逻辑
  - [x] 6.1 复用现有的 `import_openapi_with_options` 函数
    - 确保目标文件夹参数正确传递
    - _Requirements: 4.1_
  - [ ]* 6.2 为目标文件夹导入编写属性测试
    - **Property 2: 目标文件夹导入正确性**
    - **Validates: Requirements 4.1, 4.3**
  - [ ]* 6.3 为子文件夹创建编写属性测试
    - **Property 3: 子文件夹创建正确性**
    - **Validates: Requirements 4.2**

- [x] 7. 实现错误处理
  - [x] 7.1 实现输入验证
    - 验证内容非空、格式有效
    - _Requirements: 5.1_
  - [ ]* 7.2 为无效内容错误处理编写属性测试
    - **Property 4: 无效内容错误处理**
    - **Validates: Requirements 5.1**
  - [x] 7.3 实现目标文件夹验证
    - 验证文件夹存在且类型正确
    - _Requirements: 5.2_
  - [ ]* 7.4 为不存在的文件夹错误处理编写属性测试
    - **Property 5: 不存在的文件夹错误处理**
    - **Validates: Requirements 5.2**

- [x] 8. 清理旧代码
  - [x] 8.1 移除不再使用的事件类型
    - 移除 `PreviewImport`, `RequestImportPreview`, `ResolveImportConflicts`
    - _Requirements: 3.5_
  - [x] 8.2 简化 WorkspaceContent 中的事件处理
    - 移除不再需要的处理函数
    - _Requirements: 3.4_

- [x] 9. Final checkpoint - 确保所有功能正常工作
  - 确保所有测试通过，验证功能完整性，如有问题请询问用户

## Notes

- 任务标记 `*` 的为可选任务，可以跳过以实现更快的 MVP
- 每个任务都引用了具体的需求以确保可追溯性
- 检查点确保增量验证和用户反馈
- 属性测试验证通用正确性属性
- 单元测试验证具体示例和边界条件
