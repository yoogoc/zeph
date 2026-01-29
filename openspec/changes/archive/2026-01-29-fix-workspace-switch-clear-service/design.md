## Context

当前 `WorkspaceContent` 组件在订阅 `WorkspaceEvent::Open(workspace)` 时，会更新 sidebar、service_tabs、env_selector，但**未更新** `current_service` 字段。这导致切换 workspace 后，UI 仍然显示旧 workspace 的服务内容。

相关代码位置：`crates/ui/src/workspace_content.rs:48-61`

```rust
WorkspaceEvent::Open(workspace) => {
    sidebar.update(cx, |s, cx| { s.set_workspace(workspace.clone(), cx); });
    service_tabs.update(cx, |s, cx| { s.set_workspace(workspace.clone()); cx.notify(); });
    env_selector.update(cx, |s, cx| { s.set_workspace(workspace.clone()); cx.notify(); });
    // 缺失：未更新 current_service 和 http_services
}
```

## Goals / Non-Goals

**Goals:**
- 切换 workspace 时，正确同步 `current_service` 到目标 workspace 的 `active_service`
- 清理旧 workspace 的 `http_services` 缓存，避免内存泄漏和状态混乱

**Non-Goals:**
- 不修改 workspace 切换的触发机制
- 不修改 `WorkspaceRegistry` 的核心逻辑

## Decisions

### Decision 1: 在 `WorkspaceEvent::Open` 处理器中同步 service 状态

**方案**: 在现有的 `WorkspaceEvent::Open` 事件处理中，增加对 `current_service` 和 `http_services` 的更新。

**理由**: 
- 最小改动，复用现有事件机制
- 保持单一数据流：workspace 切换 → 事件触发 → UI 同步

**替代方案考虑**:
- 新增专门的 `WorkspaceSwitched` 事件：过度设计，`Open` 事件已足够
- 在 `set_view` 方法中处理：违反职责分离，`set_view` 只应处理视图切换

### Decision 2: 清空 http_services 缓存

**方案**: 切换 workspace 时清空 `http_services` HashMap。

**理由**:
- 不同 workspace 的服务 ID 可能冲突（虽然用 UUID 概率极低）
- 旧 workspace 的服务实例不应在新 workspace 中复用
- 简化状态管理，避免复杂的条件判断

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| 切换 workspace 时如果旧服务有未保存更改会丢失 | 现有行为已如此，非本次修复范围；可作为后续改进 |
| 清空缓存导致重新打开服务时需要重建 HttpService 实例 | 可接受的性能开销，保证状态正确性优先 |