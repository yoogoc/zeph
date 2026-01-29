## 1. 更新 WorkspaceEvent::Open 处理器

- [x] 1.1 在 `WorkspaceEvent::Open` 分支中，将 `this.current_service` 同步为目标 workspace 的 `active_service`
- [x] 1.2 清空 `this.http_services` HashMap，释放旧 workspace 的服务实例

## 2. 验证

- [ ] 2.1 测试：切换到无 active service 的 workspace，确认显示空状态
- [ ] 2.2 测试：切换到有 active service 的 workspace，确认显示正确服务
- [ ] 2.3 测试：在 workspace A 打开服务后切换到 B 再切回 A，确认服务正常重建