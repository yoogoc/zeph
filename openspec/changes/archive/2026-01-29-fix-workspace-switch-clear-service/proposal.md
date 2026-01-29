## Why

切换 workspace 后，UI 层的 `current_service` 状态未被重置，导致显示上一个 workspace 的遗留 service 内容。用户体验上表现为切换到新 workspace 后看到的是旧 workspace 的服务，造成困惑。

## What Changes

- 修改 workspace 切换逻辑，在切换时将 `WorkspaceContent` 的 `current_service` 置为目标 workspace 的 `active_service`（可能为 None）
- 确保 `ServiceTabs` 组件同步更新，显示目标 workspace 的已打开服务列表

## Capabilities

### New Capabilities

无新增能力。

### Modified Capabilities

无需修改现有 specs，这是纯实现层的 bug 修复。

## Impact

- **代码影响**: 
  - `crates/ui/src/workspace_content.rs` - WorkspaceContent 需要监听 workspace 切换事件
  - `crates/workspace/src/lib.rs` - 可能需要在 `set_active_workspace` 中发送额外事件
- **用户体验**: 切换 workspace 后将正确显示目标 workspace 的服务状态