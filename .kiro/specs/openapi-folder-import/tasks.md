# Implementation Plan: OpenAPI 文件夹导入功能

## Overview

本实现计划将现有的 OpenAPI 导入功能扩展为支持文件夹选择的增强版本。实现将分为 UI 组件开发、导入逻辑增强、数据模型扩展和测试验证四个主要阶段。

## Tasks

- [x] 1. 扩展数据模型和导入选项
  - 创建 ImportOptions 和相关数据结构
  - 扩展 OpenAPI 导入函数以支持文件夹选择
  - _Requirements: 2.1, 2.2, 3.2, 3.3_

- [ ]* 1.1 为导入选项编写属性测试
  - **Property 3: 默认导入行为保持一致**
  - **Validates: Requirements 1.4**

- [ ]* 1.2 为文件夹选择逻辑编写属性测试
  - **Property 4: 服务导入到指定文件夹**
  - **Validates: Requirements 2.1**

- [x] 2. 实现 FolderSelector 组件
  - 创建文件夹选择器 UI 组件
  - 实现文件夹树显示和选择逻辑
  - 添加文件夹类型过滤功能
  - _Requirements: 1.2, 1.3_

- [ ]* 2.1 为文件夹选择器编写属性测试
  - **Property 1: 文件夹选择器显示正确的文件夹**
  - **Validates: Requirements 1.2**

- [ ]* 2.2 为文件夹选择状态编写属性测试
  - **Property 2: 文件夹选择状态管理**
  - **Validates: Requirements 1.3**

- [x] 3. 实现 ImportDialog 组件
  - 创建增强的导入对话框
  - 集成文件夹选择器和导入选项
  - 实现导入预览功能
  - _Requirements: 1.1, 3.1, 5.1, 5.5_

- [ ]* 3.1 为导入预览编写属性测试
  - **Property 14: 预览服务信息准确性**
  - **Validates: Requirements 5.2**

- [ ]* 3.2 为预览模式信息编写属性测试
  - **Property 15: 预览模式信息准确性**
  - **Validates: Requirements 5.3**

- [x] 4. 增强 OpenAPI 导入逻辑
  - 修改 import_openapi 函数以支持文件夹选择
  - 实现子文件夹创建逻辑
  - 实现模式文件夹处理逻辑
  - _Requirements: 2.1, 2.2, 3.2, 3.3, 4.1, 4.2, 4.3, 4.4_

- [ ]* 4.1 为子文件夹创建编写属性测试
  - **Property 7: 子文件夹创建逻辑**
  - **Validates: Requirements 3.2**

- [ ]* 4.2 为直接导入编写属性测试
  - **Property 8: 直接导入到目标文件夹**
  - **Validates: Requirements 3.3**

- [ ]* 4.3 为文件夹层次结构编写属性测试
  - **Property 9: 子文件夹层次结构**
  - **Validates: Requirements 3.4**

- [x] 5. 实现模式导入处理
  - 实现模式文件夹检查逻辑
  - 实现模式文件夹创建和导入逻辑
  - 处理模式文件夹层次结构
  - _Requirements: 4.1, 4.2, 4.3, 4.4_

- [ ]* 5.1 为模式文件夹检查编写属性测试
  - **Property 10: 模式文件夹检查**
  - **Validates: Requirements 4.1**

- [ ]* 5.2 为模式导入编写属性测试
  - **Property 11: 模式导入到现有文件夹**
  - **Validates: Requirements 4.2**

- [ ]* 5.3 为模式文件夹创建编写属性测试
  - **Property 12: 模式文件夹创建**
  - **Validates: Requirements 4.3**

- [x] 6. 实现错误处理和验证
  - 添加输入验证逻辑
  - 实现错误消息和错误处理
  - 实现名称冲突处理
  - _Requirements: 6.1, 6.2, 6.4_

- [ ]* 6.1 为错误处理编写属性测试
  - **Property 6: 无效文件夹错误处理**
  - **Validates: Requirements 2.3**

- [ ]* 6.2 为错误消息编写属性测试
  - **Property 17: 错误消息准确性**
  - **Validates: Requirements 6.1, 6.2**

- [ ]* 6.3 为名称冲突处理编写属性测试
  - **Property 18: 名称冲突处理**
  - **Validates: Requirements 6.4**

- [x] 7. 集成到现有 UI 系统
  - 修改 WorkspaceSidebar 以使用新的导入对话框
  - 更新 WorkspaceContent 以处理新的导入事件
  - 更新 WorkspaceRegistry 以支持新的导入方法
  - _Requirements: 1.1, 2.1_

- [ ]* 7.1 为 UI 集成编写单元测试
  - 测试对话框显示和交互
  - 测试事件处理和状态更新

- [x] 8. Checkpoint - 确保所有测试通过
  - 确保所有测试通过，如有问题请询问用户

- [x] 9. 实现预览功能增强
  - 完善导入预览的显示逻辑
  - 实现文件夹路径显示
  - 添加预览数据的详细信息
  - _Requirements: 5.2, 5.3, 5.4_

- [ ]* 9.1 为预览路径显示编写属性测试
  - **Property 16: 预览文件夹路径显示**
  - **Validates: Requirements 5.4**

- [x] 10. 性能优化和最终测试
  - 优化大型 OpenAPI 规范的处理性能
  - 优化文件夹选择器的响应性能
  - 进行端到端集成测试
  - _Requirements: All_

- [ ]* 10.1 编写集成测试
  - 测试完整的导入流程
  - 测试各种边界条件和错误场景

- [x] 11. Final checkpoint - 确保所有功能正常工作
  - 确保所有测试通过，验证功能完整性，如有问题请询问用户

## Notes

- 任务标记 `*` 的为可选任务，可以跳过以实现更快的 MVP
- 每个任务都引用了具体的需求以确保可追溯性
- 检查点确保增量验证和用户反馈
- 属性测试验证通用正确性属性
- 单元测试验证具体示例和边界条件