# Implementation Plan: Environment Base URL Integration

## Overview

实现环境 base_url 与 HTTP 服务的集成，包括 URL 组合逻辑、UI 开关和数据传递。

## Tasks

- [x] 1. 创建 URL 工具模块
  - [x] 1.1 在 `crates/ui/src/http_service/` 创建 `url_utils.rs` 模块
    - 实现 `is_absolute_url` 函数判断绝对 URL
    - 实现 `combine_url` 函数组合 base_url 和 path
    - 实现 `resolve_url` 函数根据模式计算最终 URL
    - _Requirements: 3.1, 3.2, 3.3, 3.4_
  - [ ]* 1.2 编写 URL 组合属性测试
    - **Property 1: URL Combination Correctness**
    - **Validates: Requirements 3.1, 3.2, 3.3, 3.4**

- [x] 2. 扩展 HttpConfig 模型
  - [x] 2.1 在 `crates/models/src/lib.rs` 的 HttpConfig 中添加 `use_base_url` 字段
    - 添加 `#[serde(default = "default_use_base_url")]` 属性
    - 实现默认值函数返回 `true`
    - _Requirements: 2.4_
  - [ ]* 2.2 编写 HttpConfig 序列化属性测试
    - **Property 3: URL Mode Persistence Round-Trip**
    - **Validates: Requirements 2.4**

- [x] 3. 更新 HttpService 组件
  - [x] 3.1 修改 `HttpService::new` 接收 base_url 参数
    - 添加 `base_url: String` 字段
    - 添加 `use_base_url: bool` 字段（从 config 读取）
    - _Requirements: 1.1, 1.3_
  - [x] 3.2 实现 `set_base_url` 方法
    - 更新 base_url 字段
    - 调用 `cx.notify()` 触发重渲染
    - _Requirements: 1.2_
  - [x] 3.3 实现 `final_url` 方法
    - 使用 `url_utils::resolve_url` 计算最终 URL
    - _Requirements: 3.1, 3.2, 3.3, 3.4_
  - [x] 3.4 修改 `send_request` 使用 `final_url`
    - 在请求信息中记录最终 URL
    - 标注是否使用了 base_url
    - _Requirements: 5.1, 5.2_

- [x] 4. 更新 RequestPanel UI
  - [x] 4.1 添加 base_url 模式开关按钮
    - 在 URL 输入框旁添加切换按钮
    - 显示当前模式状态
    - _Requirements: 2.1, 2.2_
  - [x] 4.2 添加 `ToggleBaseUrl` 事件
    - 在 `RequestPanelEvent` 中添加新事件
    - 在 HttpService 中处理该事件
    - _Requirements: 2.1_
  - [x] 4.3 显示最终 URL 预览
    - 当 base_url 模式启用时显示组合后的 URL
    - _Requirements: 4.1_

- [x] 5. 数据传递集成
  - [x] 5.1 修改 WorkspaceContent 传递 base_url 给 HttpService
    - 从 workspace.active_environment_id 获取当前环境
    - 提取 base_url 传递给 HttpService
    - _Requirements: 1.1_
  - [x] 5.2 处理环境切换时的 base_url 更新
    - 监听环境变化事件
    - 调用 HttpService::set_base_url 更新
    - _Requirements: 1.2_

- [x] 6. Checkpoint - 确保所有测试通过
  - 运行 `cargo test`
  - 确保所有测试通过，如有问题请询问用户

- [ ]* 7. 编写集成属性测试
  - [ ]* 7.1 编写 URL 模式禁用测试
    - **Property 2: URL Mode Disabled Bypass**
    - **Validates: Requirements 2.3**
  - [ ]* 7.2 编写请求信息准确性测试
    - **Property 4: Request Info URL Accuracy**
    - **Validates: Requirements 5.1**

## Notes

- 标记 `*` 的任务为可选任务，可跳过以加快 MVP 开发
- 每个任务都引用了具体的需求以便追溯
- 属性测试验证核心正确性属性
- 单元测试验证具体示例和边界情况
