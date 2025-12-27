# Implementation Plan: WorkspaceSidebar Refactor

## Overview

重构WorkspaceSidebar组件，实现树形缩进显示和右键菜单功能。采用增量开发方式，先实现核心的树形显示功能，再添加菜单交互功能，最后进行测试和优化。

## Tasks

- [x] 1. 重构树形项目渲染结构
  - 创建TreeItem结构体和相关类型定义
  - 实现基础的树形项目渲染逻辑
  - 添加缩进计算功能
  - _Requirements: 1.1, 1.2, 1.3_

- [ ]* 1.1 Write property test for tree indentation calculation
  - **Property 1: 树形缩进计算**
  - **Validates: Requirements 1.1, 1.2**

- [ ]* 1.2 Write property test for folder expand/collapse icons
  - **Property 2: 文件夹展开状态图标**
  - **Validates: Requirements 1.3**

- [x] 2. 实现ContextMenuButton组件
  - 创建ContextMenuButton结构体
  - 实现菜单按钮的显示/隐藏逻辑
  - 添加悬停状态检测
  - _Requirements: 2.1, 2.2, 2.3, 2.4_

- [ ]* 2.1 Write property test for menu button hover visibility
  - **Property 3: 菜单按钮悬停显示**
  - **Validates: Requirements 2.1, 2.3**

- [ ]* 2.2 Write property test for menu button click response
  - **Property 4: 菜单按钮点击响应**
  - **Validates: Requirements 2.2**

- [x] 3. 实现菜单选项和事件处理
  - 定义不同类型项目的菜单选项
  - 实现菜单选择后的事件发送逻辑
  - 添加新的WorkspaceSidebarEvent变体
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 4.1, 4.2, 4.3_

- [ ]* 3.1 Write unit tests for folder menu options
  - Test folder menu contains create, import, and delete options
  - _Requirements: 3.1, 3.2_

- [ ]* 3.2 Write unit tests for service and schema menu options
  - Test service and schema menus contain delete option
  - _Requirements: 4.1, 4.2_

- [ ]* 3.3 Write property test for delete event data integrity
  - **Property 6: 删除事件数据完整性**
  - **Validates: Requirements 4.3, 5.2**

- [x] 4. 集成菜单功能到主组件
  - 更新WorkspaceSidebar的render方法
  - 集成TreeItem和ContextMenuButton组件
  - 实现菜单事件的处理和转发
  - _Requirements: 5.1, 5.3, 5.4_

- [ ]* 4.1 Write property test for create event data integrity
  - **Property 7: 创建事件数据完整性**
  - **Validates: Requirements 5.1**

- [ ]* 4.2 Write property test for import event data integrity
  - **Property 8: 导入事件数据完整性**
  - **Validates: Requirements 5.3**

- [ ]* 4.3 Write property test for menu event context completeness
  - **Property 9: 菜单事件上下文完整性**
  - **Validates: Requirements 5.4**

- [x] 5. 实现视觉反馈和交互状态
  - 添加悬停、活跃、点击状态的样式
  - 实现项目类型特定的菜单样式
  - 优化交互反馈的视觉效果
  - _Requirements: 6.1, 6.2, 6.3, 2.4_

- [ ]* 5.1 Write property test for visual feedback consistency
  - **Property 10: 视觉反馈状态一致性**
  - **Validates: Requirements 6.1, 6.2, 6.3**

- [ ]* 5.2 Write property test for menu style differentiation
  - **Property 5: 项目类型菜单样式**
  - **Validates: Requirements 2.4**

- [x] 6. 实现响应式布局和文本处理
  - 添加响应式宽度调整逻辑
  - 实现长文本的省略号截断
  - 确保深层级项目的菜单按钮可见性
  - _Requirements: 7.1, 7.2, 7.3_

- [ ]* 6.1 Write property test for responsive width adjustment
  - **Property 11: 响应式宽度调整**
  - **Validates: Requirements 7.1, 7.3**

- [ ]* 6.2 Write property test for text truncation
  - **Property 12: 长文本截断显示**
  - **Validates: Requirements 7.2**

- [x] 7. Checkpoint - 确保所有测试通过
  - 确保所有测试通过，如有问题请询问用户

- [x] 8. 错误处理和边界条件
  - 实现无效项目ID的错误处理
  - 添加深度溢出的限制逻辑
  - 处理菜单渲染失败的降级方案
  - _Requirements: Error Handling_

- [ ]* 8.1 Write unit tests for error handling scenarios
  - Test invalid project ID handling
  - Test depth overflow limits
  - Test menu rendering failure fallbacks

- [ ] 9. 性能优化和最终集成
  - 优化大量项目时的渲染性能
  - 确保所有组件正确集成
  - 验证与现有功能的兼容性
  - _Requirements: All requirements integration_

- [ ]* 9.1 Write integration tests for complete workflows
  - Test folder creation to service addition workflow
  - Test OpenAPI import and tree structure update
  - Test delete operations and UI state updates

- [ ] 10. Final checkpoint - 确保所有测试通过
  - 确保所有测试通过，如有问题请询问用户

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation
- Property tests validate universal correctness properties using fast-check or similar Rust property testing library
- Unit tests validate specific examples and edge cases
- Integration tests ensure complete workflows function correctly